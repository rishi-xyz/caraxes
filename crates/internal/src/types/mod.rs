//! Provider neutral domain types
//!
//! Nothing in this mod wll know about a specific proviuder wire format like openai compactible
//! Provider adapters translate between these types and thier own json shape / format  

use serde::{Deserialize, Serialize};

// internal types to be used acroos crates and apps

/// # Role
///
/// used to define who authored a chat message.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// A tool call the model asked for
///
/// The model never executes anything it only names a tool and args
/// caraxes decies whether and how to run it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Provider assigned id, echoed back on the matching tool results.
    pub id: String,
    /// The tool's name.
    pub name: String,
    /// json encoded args
    pub arguments: String,
}

impl ToolCall {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: impl Into<String>,
    ) -> Self {
        ToolCall {
            id: id.into(),
            name: name.into(),
            arguments: arguments.into(),
        }
    }

    /// Decode [`arguments`](Self::arguments) into a JSON value.
    ///
    /// Fails when model produced args that are not valid JSON.
    pub fn parse_arguments(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(&self.arguments)
    }
}

/// # Messages
///
/// A single message sent to the model
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    /// creating a new chat message
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Message {
            role,
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn assistant_tool_calls(tool_calls: Vec<ToolCall>) -> Self {
        Message {
            role: Role::Assistant,
            content: None,
            tool_calls,
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Message {
            role: Role::Tool,
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
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
pub enum FinishReason {
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
