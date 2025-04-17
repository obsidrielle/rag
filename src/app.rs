use clap::Parser;
use crate::config::Config;
use crate::manager::ContextManager;
use crate::processor::Processor;
use crate::tools::ToolRegistry;

#[derive(Parser)]
#[command(author = "obsidrielle", version = "1.0.0", about = "rust LLM ag(ent) for everything.", long_about = None)]
pub struct App {
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
    pub fn new() -> Self {
        Self {
            set_api_key: None,
            set_base_url: None,
            set_model: None,
        }
    }

    pub async fn run(&mut self, mut context: Context, mut processor: Processor) -> anyhow::Result<()> {
        if let Some(ref e) = self.set_model {
            context.config.model = e.to_string();
        }
        if let Some(ref e) = self.set_base_url {
            context.config.base_url = e.to_string();
        }
        if let Some(ref e) = self.set_api_key {
            context.config.api_key = e.to_string();
        }
        if self.set_api_key.is_some() || self.set_base_url.is_some() || self.set_model.is_some() {
            context.config.save_config();
            std::process::exit(0);
        }

        processor.run(&mut context).await
    }
}

pub(crate) struct Context {
    pub config: Config,
    pub manager: ContextManager,
    pub tools: ToolRegistry,
}

impl Context {
    pub fn new(config: Config, context_manager: ContextManager) -> Self {
        Self {
            config,
            manager: context_manager,
            tools: ToolRegistry::new(),
        }
    }
}