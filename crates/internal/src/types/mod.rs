//! Provider neutral domain types
//! 
//! Nothing in this mod wll know about a specific proviuder wire format like openai compactible
//! Provider adapters translate between these types and thier own json shape / format  

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

/// # Messages
/// 
/// A single message sent to the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    role: Role,
    content: String,
}


impl Message {
    /// creating a new chat message
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Message { 
            role, 
            content: content.into() 
        }
    }
}

/// # Usage
/// 
/// Token accounting reported by a provider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

/// # FinishReason
/// 
/// the reason why model stopped generating output
pub enum FinishReason{
    /// the generation successfully completed
    Stop,
    /// the generation length has been reached
    Length,
    /// Tool calls is being done
    ToolCalls,
    /// Content filter is being performed
    ContentFilter,
    /// Any value the provider sends that we do not model yet
    Other(String),
}

impl FinishReason {
    /// Parse a provider `finish_reason` string. This never fails
    pub fn parse_from_provider(value: &str) -> Self {
        match value {
            "stop" => FinishReason::Stop,
            "length" => FinishReason::Length,
            "tool_calls" => FinishReason::ToolCalls,
            "content_filter" => FinishReason::ContentFilter,
            other => FinishReason::Other(other.to_string()),
        }
    }
}