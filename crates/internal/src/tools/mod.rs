//! Tool definitions and the registry that holds them.
//!
//! This module have tool definition, registry and result

use crate::types::Message;
use serde::Serialize;
use serde_json::Value;

/// A tool the model is allowed to ask for
///
/// `params` is a json scheme object describing the args, exactly as the model needs to see it
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub params: Value,
}

impl ToolDefinition {
    pub fn new(name: impl Into<String>, description: impl Into<String>, params: Value) -> Self {
        ToolDefinition {
            name: name.into(),
            description: description.into(),
            params,
        }
    }
}

/// Set of tools available to the model, looked up by name
///
/// Registration order is preserved so request are deterministic
#[derive(Debug, Clone, Default)]
pub struct ToolRegistry {
    tools: Vec<ToolDefinition>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        ToolRegistry { tools: Vec::new() }
    }

    /// Add a tool.
    ///
    /// the later the tool the faster the lookup time, this is done as basic or core  tool will already be there and added at prompt and loaded first
    /// also a later tool with the same name wins at lookup will be good for retrival (saw on x), but both are kept so a duplicate is visible.
    pub fn register(&mut self, tool: ToolDefinition) {
        self.tools.push(tool);
    }

    /// get a tool
    ///
    /// the later the tool the faster the lookup
    pub fn get(&self, name: &str) -> Option<&ToolDefinition> {
        self.tools.iter().rev().find(|tool| tool.name == name)
    }

    /// All registered definitions. in order
    pub fn definitions(&self) -> &[ToolDefinition] {
        &self.tools
    }

    /// gives len maybe needed for display and utils
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

/// outcome of the tool
#[derive(Debug, Clone, PartialEq)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    pub is_error: bool, //Not sure if we should add it //CHECK
}

impl ToolResult {
    pub fn success(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        ToolResult {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
            is_error: false,
        }
    }

    pub fn error(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        ToolResult {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
            is_error: true,
        }
    }

    /// fn to turn reult into message the model expects
    pub fn into_message(self) -> Message {
        Message::tool_result(self.tool_call_id, self.content)
    }
}
