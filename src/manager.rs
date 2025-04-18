use async_openai::types::{ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs};

#[derive(Debug, Default)]
pub(crate) struct ContextManager {
    contexts: Vec<ChatCompletionRequestMessage>,
    max_size: usize,
}

impl ContextManager {
    pub fn new(max_size: usize) -> Self {
        Self {
            contexts: vec![],
            max_size,
        }
    }

    fn shift(&mut self) {
        self.contexts.remove(1);
        self.contexts.remove(1);
    }

    pub fn add(&mut self, message: ChatCompletionRequestMessage) {
        if self.contexts.len() == self.max_size { self.shift(); }
        self.contexts.push(message); 
    }

    pub fn as_messages<'a>(&mut self) -> Vec<ChatCompletionRequestMessage> {
        self.contexts.clone()
    }
}