use std::fmt::Debug;
use std::fs::File;
use std::{fs, io};
use std::cell::RefCell;
use std::io::{Read, stdout, Write};
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::error::OpenAIError;
use clap::command;
use colored::Colorize;
use encoding_rs::GBK;
use futures::StreamExt;
use futures_core::Stream;
use regex::Regex;
use serde_json::{json, Value};
use crate::app::Context;
use crate::manager::Role;
use rustyline::{DefaultEditor, Editor};
use crate::rq::{RqBodyBuilder, RsChunkBody};

#[derive(Debug, Default)]
pub(crate) struct Processor {
    pre_input_hooks: Vec<Rc<dyn PreInputHook>>,
    pre_call_hooks: Vec<Rc<dyn PreCallHook>>,
    post_call_hooks: Vec<Rc<dyn PostCallHook>>,
    pre_next_input_hooks: Vec<Rc<dyn PreNextInputHook>>,
}

impl Processor {
    pub fn new(default_hooks: bool) -> Self {
        let mut process = Processor {
            pre_input_hooks: vec![],
            pre_call_hooks: vec![],
            post_call_hooks: vec![],
            pre_next_input_hooks: vec![],
        };

        if default_hooks { process.add_default_hooks(); }
        process
    }


    fn add_default_hooks(&mut self) {
        let token_tracer = Rc::new(TokenTracer::new());

        self.add_hook(Hook::PreCallHook(Rc::new(CommandParser::new())));
        self.add_hook(Hook::PreCallHook(Rc::new(AnswerPrompt)));
        self.add_hook(Hook::PostCallHook(Rc::new(ReasoningCollector)));
        self.add_hook(Hook::PostCallHook(Rc::new(ContentCollector)));
        self.add_hook(Hook::PostCallHook(token_tracer.clone()));
        self.add_hook(Hook::PreNextInputHook(token_tracer.clone()));
        self.add_hook(Hook::PreNextInputHook(Rc::new(NewLine)));
    }

    fn add_hook(&mut self, hook: Hook) {
        match hook {
            Hook::PreInputHook(hook) => self.pre_input_hooks.push(hook),
            Hook::PreCallHook(hook) => self.pre_call_hooks.push(hook),
            Hook::PostCallHook(hook) => self.post_call_hooks.push(hook),
            Hook::PreNextInputHook(hook) => self.pre_next_input_hooks.push(hook),
        }
    }

    pub async fn run(&mut self, context: &mut Context) -> anyhow::Result<()> {
        let mut rl = DefaultEditor::new()?;
        let mut base_body = RqBodyBuilder::default();
        base_body.tools(context.tools.to_tools_call_body());
        base_body.model(context.config.model.clone());

        loop {
            for e in &self.pre_input_hooks { e.pre_input(context)? }

            let mut user_input = rl.readline("🚀 ^D: ")?.trim().to_string();

            for e in &self.pre_call_hooks { e.pre_call(context, &mut user_input)? }

            context.manager.add(Role::User, &user_input);
            let rq_config = OpenAIConfig::new()
                .with_api_base(&context.config.base_url)
                .with_api_key(&context.config.api_key);
            
            let client = Client::with_config(rq_config);
            let rq_body = base_body.messages(context.manager.as_messages()).build()?;

            let mut stream: Pin<Box<dyn Stream<Item = Result<Value, OpenAIError>>>> = client.chat()
                .create_stream_byot(rq_body.to_rq_body())
                .await?;

            let mut answer = String::new();

            while let Some(result) = stream.next().await {
                if let Ok(chunk) = result {
                    let Chunk = serde_json::from_value::<RsChunkBody>(chunk.clone())?;
                    
                    if let Some(content) = chunk["choices"][0]["delta"]["content"].as_str() {
                        answer.push_str(content);
                    }
                    for e in &self.post_call_hooks { e.post_call(context, &chunk)?; }
                }
            }

            context.manager.add(Role::Assistant, &answer);
            for e in &self.pre_next_input_hooks { e.pre_next_input(context)?; }
        }
    }
}

pub enum Hook {
    PreInputHook(Rc<dyn PreInputHook>),
    PreCallHook(Rc<dyn PreCallHook>),
    PostCallHook(Rc<dyn PostCallHook>),
    PreNextInputHook(Rc<dyn PreNextInputHook>),
}

pub trait PreInputHook: Debug {
    fn pre_input(&self, ctx: &mut Context) -> anyhow::Result<()>;
}

#[derive(Debug)]
struct InitPrompt;

impl PreInputHook for InitPrompt {
    fn pre_input(&self, ctx: &mut Context) -> anyhow::Result<()> {
        let init_prompt = "🚀 ^D: ";
        print!("{}", init_prompt);
        stdout().flush()?;
        Ok(())
    }
}

pub trait PreCallHook: Debug {
    fn pre_call(&self, ctx: &mut Context, input: &mut String) -> anyhow::Result<()>;
}

#[derive(Debug)]
struct CommandParser {
    commands: Vec<Box<dyn Command>>,
}

impl CommandParser {
    pub fn new() -> Self {
        let mut parser = CommandParser {
            commands: vec![],
        };

        parser.register_command(Box::new(ExitCommand));
        parser.register_command(Box::new(FileCommand::new()));
        parser.register_command(Box::new(SystemCommand::new()));

        parser
    }

    fn register_command(&mut self, command: Box<dyn Command>) {
        self.commands.push(command);
    }
}

impl PreCallHook for CommandParser {
    fn pre_call(&self, ctx: &mut Context, input: &mut String) -> anyhow::Result<()> {
        for command in &self.commands {
            if command.is(input.as_str()) {
                command.execute(input)?;
            }
        }
        Ok(())
    }
}

trait Command: Debug {
    fn is(&self, input: &str) -> bool;

    fn execute(&self, input: &mut String) -> anyhow::Result<()>;
}

#[derive(Debug)]
struct ExitCommand;

impl Command for ExitCommand {
    fn is(&self, input: &str) -> bool {
        input.starts_with("@exit")
    }

    fn execute(&self, _input: &mut String) -> anyhow::Result<()> {
        println!("{}", "bye".yellow());
        stdout().flush()?;
        std::process::exit(0);
    }
}

#[derive(Debug)]
struct FileCommand {
    pattern: Regex,
}

impl FileCommand {
    pub fn new() -> Self {
        Self {
            pattern: Regex::new(r"@file\((?<path>[^)]+)\)").unwrap(),
        }
    }
}

impl Command for FileCommand {
    fn is(&self, input: &str) -> bool {
        self.pattern.is_match(input)
    }

    fn execute(&self, input: &mut String) -> anyhow::Result<()> {
        let result = self.pattern.replace_all(input.as_str(), |caps: &regex::Captures| {
            let file_path = Path::new(&caps["path"]);
            match fs::read_to_string(file_path) {
                Ok(content) => format!("{}: {}", &caps["path"], content),
                Err(e) => {
                    eprintln!("{}", format!("Warning: Failed to read file {}: {}", &caps["path"], e).yellow());
                    caps[0].to_string()
                }
            }
        });

        *input = result.to_string();
        Ok(())
    }
}

#[derive(Debug)]
struct SystemCommand {
    pattern: Regex,
}

impl SystemCommand {
    pub fn new() -> Self {
        Self {
            pattern: Regex::new(r"@`(?P<command>.*)`").unwrap(),
        }
    }
}
impl Command for SystemCommand {
    fn is(&self, input: &str) -> bool {
        self.pattern.is_match(input)
    }

    fn execute(&self, input: &mut String) -> anyhow::Result<()> {
        let result = self.pattern.replace_all(input.as_str(), |caps: &regex::Captures| {
            if &caps[0] == "@`(?P<command>.*)`" { return caps[0].to_string(); }

            let parts = shell_words::split(&caps["command"]).unwrap();
            let (elf, args) = parts.split_first().unwrap();

            let mut command = std::process::Command::new(elf);
            let mut output = command
                .args(args)
                .output()
                .expect("Failed to get command output");

            if cfg!(target_os = "windows") {
                println!("cmd /C {}", format!("\"{}\"", &caps["command"]));
                command = std::process::Command::new("cmd");
                output = command.arg("/C")
                    .arg(format!("\"{}\"", &caps["command"]))
                    .output()
                    .expect("Failed to get command output");
            }

            if output.status.success() {
                let stdout = match String::from_utf8(output.stdout.clone()) {
                    Ok(inner) => inner,
                    Err(_) => {
                        GBK.decode(&output.stdout).0.to_string()
                    }
                };
                stdout
            } else {
                let stderr = match String::from_utf8(output.stderr.clone()) {
                    Ok(inner) => inner,
                    Err(_) => GBK.decode(&output.stderr).0.to_string(),
                };
                let exit_code = output.status.code().unwrap_or(-1);
                eprintln!("{}", format!("Warning: Command failed with exit code {}: {}", exit_code, stderr));
                caps[0].to_string()
            }
        });
        *input = result.to_string();
        Ok(())
    }
}

#[derive(Debug)]
struct AnswerPrompt;

impl PreCallHook for AnswerPrompt {
    fn pre_call(&self, ctx: &mut Context, input: &mut String) -> anyhow::Result<()> {
        let prompt = format!("🤖 {}: ", &ctx.config.model);
        print!("{}", prompt);
        stdout().flush()?;
        Ok(())
    }
}

pub trait PreNextInputHook: Debug {
    fn pre_next_input(&self, ctx: &mut Context) -> anyhow::Result<()>;
}

pub trait PostCallHook: Debug {
    fn post_call(&self, ctx: &mut Context, chunk: &Value) -> anyhow::Result<()>;
}

#[derive(Debug)]
struct ReasoningCollector;

impl PostCallHook for ReasoningCollector {
    fn post_call(&self, ctx: &mut Context, chunk: &Value) -> anyhow::Result<()> {
        let mut lock = stdout().lock();

        if let Some(content) = chunk["choices"][0]["delta"]["reasoning_content"].as_str() {
            write!(lock, "{}", format!("{}", content).truecolor(128, 138, 135)).expect("Failed to write reasoning message");
        }

        stdout().flush()?;
        Ok(())
    }
}

#[derive(Debug)]
struct ContentCollector;

impl PostCallHook for ContentCollector {
    fn post_call(&self, ctx: &mut Context, chunk: &Value) -> anyhow::Result<()> {
        let mut lock = stdout().lock();

        if let Some(content) = chunk["choices"][0]["delta"]["content"].as_str() {
            write!(lock, "{}", content).expect("Failed to write content message");
        }

        stdout().flush()?;
        Ok(())
    }
}

#[derive(Debug)]
struct NewLine;

impl PreNextInputHook for NewLine {
    fn pre_next_input(&self, ctx: &mut Context) -> anyhow::Result<()> {
        println!();
        stdout().flush()?;
        Ok(())
    }
}

#[derive(Debug)]
struct TokenTracer {
    token_usage: RefCell<u64>,
}

impl TokenTracer {
    pub fn new() -> Self {
        Self {
            token_usage: RefCell::new(0),
        }
    }
}

impl PostCallHook for TokenTracer {
    fn post_call(&self, _ctx: &mut Context, chunk: &Value) -> anyhow::Result<()> {
        if let Some(usage) = chunk["usage"]["total_tokens"].as_u64() {
            *self.token_usage.borrow_mut() += usage;
        }
        Ok(())
    }
}

impl PreNextInputHook for TokenTracer {
    fn pre_next_input(&self, ctx: &mut Context) -> anyhow::Result<()> {
        let mut lock = stdout().lock();
        write!(lock, "{}", format!("\ntoken usage: {}", *self.token_usage.borrow_mut()).truecolor(128, 138, 135))?;
        Ok(())
    }
}