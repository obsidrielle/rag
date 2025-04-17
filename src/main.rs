use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::CreateChatCompletionRequest;
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

#[tokio::main]
async fn main() {
    let config = Config::new();
    let manager = ContextManager::new(10);
    let context = Context::new(config, manager);
    let processor = Processor::new(true);

    let mut app: App = app::App::parse();
    app.context = context;
    app.processor = processor;

    app.processor.run(&mut app.context).await.expect("Internal Error: ");
}
