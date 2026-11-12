use serde::{
    Deserialize,
    Serialize
};

// internal types to be used acroos crates and apps


/// # Role
/// 
/// used to define who authored a chat message.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all="lowercase")] 
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// # ChatMessages
/// 
/// A single message sent to the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessages {
    role: Role,
    content: String,
}


impl ChatMessages {
    /// creating a new chat message
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        ChatMessages { 
            role, 
            content: content.into() 
        }
    }
}

/// # ChatRequest
/// 
/// The request body for an OpenAI-compatible chat completion
/// 
/// messages should contain all the previous messages as well 
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessages>,
}

impl ChatRequest {
    pub fn new(model: String, messages: Vec<ChatMessages>) -> Self {
        ChatRequest { model, messages }
    }
}

/// # ChatMessageResponse
/// 
/// the message returned by the provider
/// 
/// content needs to be optional 
/// cuz provider returns `null` when model emits tool calls
/// see open ai sdk documentation for more info.
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessageResponse {
    pub content : Option<String>
}

/// # Choice 
/// 
/// A single completion choice
#[derive(Debug, Serialize, Deserialize)]
pub struct Choice {
    pub message: ChatMessageResponse,
    pub finish_reason: Option<String>,
}

/// # ChatResponse
/// 
/// The top level chat completion response.
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>
}