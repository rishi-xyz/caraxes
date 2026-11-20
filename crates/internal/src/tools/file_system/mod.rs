//! File System tools like reading file and writing file
//! 
//! Note: Just gonna added read file for now to see the whole tool calling working
//! 
//! will add other basic tool later on

use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use serde_json::json;

use crate::{tools::{ToolDefinition, ToolExecutor, ToolResult}, types::ToolCall};

pub struct ReadFile {
    root: PathBuf,
}

impl ReadFile {
    pub fn new(root: impl Into<PathBuf>) ->Self {
        ReadFile { root: root.into() }
    }

    fn resolve(&self, requested: &str) ->Result<PathBuf, String> {
        let path = Path::new(requested);

        if path.is_absolute() {
            return Err(format!("absolute paths are not allowed: {requested}"));
        }

        if path.components().any(|comp| matches!(comp, Component::ParentDir)) {
            return Err(format!("path escapes the workspace: {requested}"));
        }
        Ok(self.root.join(path))
    }
}

#[async_trait]
impl ToolExecutor for ReadFile {
    fn definition(&self) ->ToolDefinition {
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
            Err(err) => return  ToolResult::error(call.id.as_str(), format!("invalid arguments: {err}")),
            
        };

        let Some(path) = args.get("path").and_then(|value| value.as_str()) else {
            return ToolResult::error(call.id.as_str(), "missing required arguments: path");
        };

        let full = match self.resolve(path) {
            Ok(full) => full,
            Err(err) => return ToolResult::error(call.id.as_str(), err),
        };

        match  tokio::fs::read_to_string(&full).await {
            Ok(content) => ToolResult::success(call.id.as_str(), content),
            Err(err) => ToolResult::error(call.id.as_str(), format!("failed to read {path}: {err}")),
            
        }
    }
}