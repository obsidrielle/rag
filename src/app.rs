use clap::Parser;
use crate::config::Config;
use crate::manager::ContextManager;
use crate::processor::Processor;

#[derive(Parser, Default, Debug)]
#[command(author = "obsidrielle", version = "1.0.0", about = "rust LLM ag(ent) for everything.", long_about = None)]
pub struct App {
    #[clap(skip)]
    pub processor: Processor,
    #[clap(skip)]
    pub context: Context,
    /// Set api key and exit
    #[arg(long = "sa")]
    set_api_key: Option<String>,
    /// Set model and exit
    #[arg(long = "sm")]
    set_model: Option<String>,
    /// Set base url and exit
    #[arg(long = "sb")]
    set_base_url: Option<String>,
}

impl App {
    pub fn new(processor: Processor, context: Context) -> Self {
        Self {
            processor,
            context,
            ..App::default()
        }
    }

    pub fn run(&mut self) {
        if let Some(ref e) = self.set_model {
            self.context.config.model = e.to_string();
        }
        if let Some(ref e) = self.set_base_url {
            self.context.config.base_url = e.to_string();
        }
        if let Some(ref e) = self.set_api_key {
            self.context.config.api_key = e.to_string();
        }
        if self.set_api_key.is_some() || self.set_base_url.is_some() || self.set_model.is_some() {
            self.context.config.save_config();
            std::process::exit(0);
        }


    }
}

#[derive(Default, Debug)]
pub(crate) struct Context {
    pub config: Config,
    pub manager: ContextManager,
}

impl Context {
    pub fn new(config: Config, context_manager: ContextManager) -> Self {
        Self {
            config,
            manager: context_manager,
        }
    }
}