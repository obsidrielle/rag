use async_openai::types::{ChatCompletionMessageToolCallChunk, ChatCompletionRequestMessage, FinishReason, FunctionCall};
use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Builder, Serialize)]
pub struct RqBody {
    pub model: String,
    pub messages: Vec<ChatCompletionRequestMessage>,
    #[builder(default = "true")]
    pub stream: bool,
    #[builder(default)]
    pub stream_options: StreamOptions,
    #[builder(default = None)]
    pub tools: Option<Value>,
    #[builder(default = "auto".to_string())]
    pub tool_choice: String,
}

#[derive(Debug, Clone, Builder, Serialize)]
struct StreamOptions {
    #[builder(default = "true")]
    pub include_usage: bool,
}

impl Default for StreamOptions {
    fn default() -> Self {
        Self {
            include_usage: true,
        }
    }
}

impl RqBody {
    pub fn to_rq_body(self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

#[derive(Debug, Deserialize)]
pub struct RsChunkBody {
    pub id: String,
    pub choices: Vec<Choice>,
    pub created: u64,
    pub model: String,
    pub system_fingerprint: Option<String>,
    pub object: String,
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub delta: Delta,
    pub finish_reason: Option<FinishReason>,
    pub index: u64,
}

#[derive(Debug, Deserialize)]
pub struct Delta {
    pub content: String,
    pub reasoning_content: Option<String>,
    pub role: String,
    pub tool_calls: Option<Vec<ChatCompletionMessageToolCallChunk>>,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub completion_tokens: u64,
    pub prompt_tokens: u64,
    pub prompt_cache_hit_tokens: Option<u64>,
    pub prompt_cache_miss_tokens: Option<u64>,
    pub total_tokens: u64,
    pub completion_tokens_details: Option<CompletionTokensDetails>
}

#[derive(Debug, Deserialize)]
pub struct CompletionTokensDetails {
    pub reasoning_tokens: u64,
}