use std::fmt::Debug;
use std::fs;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{stdout, Write};
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::error::OpenAIError;
use async_openai::types::{ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestFunctionMessageArgs, ChatCompletionRequestToolMessageArgs, ChatCompletionRequestUserMessageArgs, ChatCompletionResponseMessage};
use colored::Colorize;
use encoding_rs::GBK;
use futures::StreamExt;
use futures_core::Stream;
use regex::Regex;
use serde_json::{json, Value};
use crate::app::Context;
use rustyline::{CompletionType, Config, DefaultEditor, EditMode, Editor};
use rustyline::hint::HistoryHinter;
use rustyline::validate::MatchingBracketValidator;
use crate::rl_helper::RlHelper;
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
        let tools_executor = Rc::new(ToolsExecutor::new());

        self.add_hook(Hook::PreCallHook(Rc::new(CommandParser::new())));
        self.add_hook(Hook::PreCallHook(Rc::new(AnswerPrompt)));
        self.add_hook(Hook::PostCallHook(Rc::new(ReasoningCollector)));
        self.add_hook(Hook::PostCallHook(Rc::new(ContentCollector)));
        self.add_hook(Hook::PostCallHook(tools_executor.clone()));
        self.add_hook(Hook::PostCallHook(token_tracer.clone()));
        self.add_hook(Hook::PreNextInputHook(tools_executor.clone()));
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
        let mut rl = RlHelper::new_rl()?;
        let prompt = "🌟 ^D:".blue().bold().to_string();

        loop {
            for e in &self.pre_input_hooks { e.pre_input(context)? }

            let mut user_input = rl.readline(&prompt)?.trim().to_string();

            for e in &self.pre_call_hooks { e.pre_call(context, &mut user_input)? }

            context.manager.add(ChatCompletionRequestUserMessageArgs::default()
                .content(user_input.as_str())
                .build()?
                .into());

            let rq_body = context
                .rq_body
                .messages(context.manager.as_messages())
                .build()?;

            // println!("{}", serde_json::to_string_pretty(&rq_body)?);

            let mut stream: Pin<Box<dyn Stream<Item = Result<Value, OpenAIError>>>> = context
                .client
                .chat()
                .create_stream_byot(rq_body.to_rq_body())
                .await?;

            let mut answer = String::new();

            while let Some(result) = stream.next().await {
                // println!("{:?}", result);
                if let Ok(chunk) = result {
                    let chunk = serde_json::from_value::<RsChunkBody>(chunk.clone())?;

                    if !chunk.choices.is_empty() {
                        answer.push_str(chunk.choices[0].delta.content.as_str());
                    }

                    for e in &self.post_call_hooks { e.post_call(context, &chunk)?; }
                }
            }

            context.manager.add(ChatCompletionRequestAssistantMessageArgs::default()
                .content(answer)
                .build()?
                .into());
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
                eprintln!("{}", format!("Warning: Command {}, failed with exit code {}: {}", &caps["command"], exit_code, stderr));
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
    fn pre_call(&self, ctx: &mut Context, _input: &mut String) -> anyhow::Result<()> {
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
    fn post_call(&self, ctx: &mut Context, chunk: &RsChunkBody) -> anyhow::Result<()>;
}

#[derive(Debug)]
struct ReasoningCollector;

impl PostCallHook for ReasoningCollector {
    fn post_call(&self, _ctx: &mut Context, chunk: &RsChunkBody) -> anyhow::Result<()> {
        let mut lock = stdout().lock();

        if chunk.choices.is_empty() {
            return Ok(());
        }

        if let Some(ref content) = chunk.choices[0].delta.reasoning_content {
            write!(lock, "{}", format!("{}", content).truecolor(128, 138, 135)).expect("Failed to write reasoning message");
        }

        stdout().flush()?;
        Ok(())
    }
}

#[derive(Debug)]
struct ContentCollector;

impl PostCallHook for ContentCollector {
    fn post_call(&self, _ctx: &mut Context, chunk: &RsChunkBody) -> anyhow::Result<()> {
        let mut lock = stdout().lock();

        if chunk.choices.is_empty() {
            return Ok(());
        }

        let content = &chunk.choices[0].delta.content;
        write!(lock, "{}", content).expect("Failed to write content message");

        stdout().flush()?;
        Ok(())
    }
}

#[derive(Debug)]
struct NewLine;

impl PreNextInputHook for NewLine {
    fn pre_next_input(&self, _ctx: &mut Context) -> anyhow::Result<()> {
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
    fn post_call(&self, _ctx: &mut Context, chunk: &RsChunkBody) -> anyhow::Result<()> {
        if let Some(usage) = &chunk.usage {
            *self.token_usage.borrow_mut() += usage.total_tokens;
        }
        Ok(())
    }
}

impl PreNextInputHook for TokenTracer {
    fn pre_next_input(&self, _ctx: &mut Context) -> anyhow::Result<()> {
        let mut lock = stdout().lock();
        write!(lock, "{}", format!("\ntoken usage: {}", *self.token_usage.borrow_mut()).truecolor(128, 138, 135))?;
        Ok(())
    }
}

#[derive(Debug)]
struct ToolsExecutor {
    tools_call: RefCell<HashMap<u32, (String, String)>>
}

impl ToolsExecutor {
    pub fn new() -> Self {
        Self {
            tools_call: RefCell::new(HashMap::new()),
        }
    }
}

impl PostCallHook for ToolsExecutor {
    fn post_call(&self, _ctx: &mut Context, chunk: &RsChunkBody) -> anyhow::Result<()> {
        if chunk.choices.is_empty() { return Ok(()); }
        if let Some(ref tool_calls) = chunk.choices[0].delta.tool_calls {
            for tool_call in tool_calls {
                if let Some(ref function) = tool_call.function {
                    if let Some(ref name) = function.name {
                        self.tools_call.borrow_mut().insert(tool_call.index, (name.to_owned(), String::new()));
                    }
                    if let Some(ref arguments) = function.arguments {
                        self.tools_call
                            .borrow_mut()
                            .entry(tool_call.index)
                            .and_modify(|(_, tool_arguments)| {
                                tool_arguments.push_str(arguments.as_str());
                            });
                    }
                }
            }
        }

        Ok(())
    }
}

impl PreNextInputHook for ToolsExecutor {
    fn pre_next_input(&self, ctx: &mut Context) -> anyhow::Result<()> {
        if self.tools_call.borrow().is_empty() {
            return Ok(());
        }

        for (index, (tool_name, arguments)) in self.tools_call.borrow().iter() {
            println!("{}", format!("Info: call tools {}, with arguments {}", tool_name, arguments).truecolor(128, 138, 135));
            let result = ctx.tools.execute(
                tool_name,
                serde_json::from_str(arguments.as_str())?
            )?;

            ctx.manager.add(ChatCompletionRequestToolMessageArgs::default()
                .content(serde_json::to_string(&result)?)
                .tool_call_id(index.to_string())
                .build()?
                .into());
        }

        let rq_body = ctx.rq_body.messages(ctx.manager.as_messages()).build()?;
        let client = ctx.client.clone();

        futures::executor::block_on(async move {
            let mut stream: Pin<Box<dyn Stream<Item = Result<Value, OpenAIError>>>> = client
                .chat()
                .create_stream_byot(rq_body.to_rq_body())
                .await
                .unwrap();

            while let Some(result) = stream.next().await {
                if let Ok(chunk) = result {
                    let chunk = serde_json::from_value::<RsChunkBody>(chunk.clone()).expect("Failed to parse chunk");

                    if chunk.choices.is_empty() { continue; }

                    let mut lock = stdout().lock();

                    if let Some(ref reasoning_content) = chunk.choices[0].delta.reasoning_content {
                        write!(lock, "{}", format!("{}", reasoning_content).truecolor(128, 138, 135)).expect("Failed to write reasoning message");
                    }

                    let content = &chunk.choices[0].delta.content;
                    write!(lock, "{}", content).expect("Failed to write content message");
                    stdout().flush().expect("Failed to flush stdout");
                }
            }
        });

        self.tools_call.borrow_mut().clear();
        Ok(())
    }
}