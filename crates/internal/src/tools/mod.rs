//! Tool definitions and the registry that holds them.
//!
//! This module have tool definition, registry and result

pub mod read_file;
pub mod run_command;

use std::sync::Arc;

use crate::types::{Message, ToolCall};
use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use tracing::{debug, instrument};

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

/// Behaviour control of a tool
///
/// tool will spwan processes so its async, a trait cuz runtime will not know which tool exists
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// how this tool is seen by the model
    fn definition(&self) -> ToolDefinition;
    /// run the tool , failure are returned so model can see what went wrong
    async fn execute(&self, call: &ToolCall) -> ToolResult;
}

struct RegisteredTool {
    definition: ToolDefinition,
    executor: Arc<dyn ToolExecutor>,
}

/// Set of tools available to the model, looked up by name
///
/// Registration order is preserved so request are deterministic
#[derive(Default)]
pub struct ToolRegistry {
    tools: Vec<RegisteredTool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        ToolRegistry { tools: Vec::new() }
    }

    /// Add a tool.
    ///
    /// the later the tool the faster the lookup time, this is done as basic or core  tool will already be there and added at prompt and loaded first
    /// also a later tool with the same name wins at lookup will be good for retrival (saw on x), but both are kept so a duplicate is visible.
    pub fn register(&mut self, executor: Arc<dyn ToolExecutor>) {
        let definition = executor.definition();
        self.tools.push(RegisteredTool {
            definition,
            executor,
        });
    }

    /// get a tool
    ///
    /// the later the tool the faster the lookup
    pub fn get(&self, name: &str) -> Option<&ToolDefinition> {
        self.tools
            .iter()
            .rev()
            .find(|tool| tool.definition.name == name)
            .map(|tool| &tool.definition)
    }

    /// All registered definitions. in order
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .map(|tool| tool.definition.clone())
            .collect()
    }

    /// gives len maybe needed for display and utils
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    #[instrument(skip_all)]
    pub async fn execute(&self, call: &ToolCall) -> ToolResult {
        debug!(tool = %call.name, id = %call.id, "dispatching tool call");

        let result = match self
            .tools
            .iter()
            .rev()
            .find(|tool| tool.definition.name == call.name)
        {
            Some(tool) => tool.executor.execute(call).await,
            None => ToolResult::error(call.id.as_str(), format!("unknown tool: {}", call.name)),
        };

        debug!(tool = %call.name, is_error = result.is_error, "tool call returned");
        result
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
