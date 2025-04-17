use async_openai::types::{ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage, ChatCompletionRequestUserMessageArgs};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Default)]
pub(crate) struct ContextManager {
    contexts: Vec<ChatContext>,
    max_size: usize,
}

impl ContextManager {
    pub fn new(max_size: usize) -> Self {
        Self {
            contexts: vec![ChatContext {
                role: Role::System,
                message: "You are an expert in the field of the questions I asked and you gave comprehensive and insightful answers.".into(),
            }],
            max_size,
        }
    }

    fn shift(&mut self) {
        self.contexts.remove(1);
        self.contexts.remove(1);
    }

    pub fn add(&mut self, role: impl Into<Role>, message: impl Into<String>) {
        if self.contexts.len() - 1 == self.max_size { self.shift(); }
        let role = role.into();
        let message = message.into();

        self.contexts.push(ChatContext {
            role,
            message
        });
    }

    pub fn as_messages<'a>(&mut self) -> serde_json::Value {
        serde_json::to_value(&self.contexts).unwrap()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
#[derive(Debug, Default)]
pub enum Role {
    #[default] User,
    Assistant,
    System,
}

impl From<&str> for Role {
    fn from(value: &str) -> Self {
        match value {
            "User" => Self::User,
            "Assistant" => Self::Assistant,
            "System" => Self::System,
            _ => unreachable!(),
        }
    }
}

#[derive(Serialize)]
#[derive(Debug, Default)]
struct ChatContext {
    role: Role,
    #[serde(rename = "content")]
    message: String,
}

// lazy_static! {
//     static ref CONTEXT_MANAGER: ContextManager = {
//         ContextManager::new(10)
//     };
// }