use async_openai::Client;
use async_openai::config::OpenAIConfig;
use crate::app::{App, Context};
use crate::config::Config;
use crate::manager::ContextManager;
use crate::processor::Processor;

use crate::tools::ToolParameters;
use clap::Parser;

mod config;
mod manager;
mod processor;
mod app;
mod tools;
mod rq;
mod rl_helper;

#[tokio::main]
async fn main() {
    let config = Config::new();
    let manager = ContextManager::new(10);

    let rq_config = OpenAIConfig::new()
        .with_api_base(config.base_url.clone())
        .with_api_key(config.api_key.clone());

    let client = Client::with_config(rq_config);

    let context = Context::new(config, manager, client);
    let processor = Processor::new(true);

    let mut app: App = app::App::parse();
    app.run(context, processor).await.expect("Internal Error: "); 
}
