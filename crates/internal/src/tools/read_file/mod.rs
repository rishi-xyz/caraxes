//! File System tools like reading file and writing file
//!
//! Note: Just gonna added read file for now to see the whole tool calling working
//!
//! will add other basic tool later on

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use crate::{
    error::Error::Io,
    exec::ExecutionRuntime,
    tools::{ToolDefinition, ToolExecutor, ToolResult},
    types::ToolCall,
};

pub struct ReadFile {
    runtime: Arc<ExecutionRuntime>,
}

impl ReadFile {
    pub fn new(runtime: Arc<ExecutionRuntime>) -> Self {
        ReadFile { runtime }
    }
}

#[async_trait]
impl ToolExecutor for ReadFile {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "read_file",
            "Read a UTF-8 tetx file from the workspace. absolute paths aren't allowed and path that escapes the workspace aren't allowed",
            json!({
                "type":"object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Paht ot the file, relative ot the workspace root."
                    }
                },
                "required": ["path"]
            }),
        )
    }

    async fn execute(&self, call: &ToolCall) -> ToolResult {
        let args = match call.parse_arguments() {
            Ok(args) => args,
            Err(err) => {
                return ToolResult::error(call.id.as_str(), format!("invalid arguments: {err}"));
            }
        };

        let Some(path) = args.get("path").and_then(|value| value.as_str()) else {
            return ToolResult::error(call.id.as_str(), "missing required arguments: path");
        };

        match self.runtime.read_file(path).await {
            Ok(contents) => ToolResult::success(call.id.as_str(), contents),
            Err(Io(err)) => ToolResult::error(call.id.as_str(), err.to_string()),
            Err(err) => ToolResult::error(call.id.as_str(), err.to_string()),
        }
    }
}
