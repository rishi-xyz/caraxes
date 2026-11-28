use std::{sync::Arc, time::Duration};

use serde_json::json;
use tokio_util::sync::CancellationToken;

#[cfg(unix)]
use crate::exec::process::CommandSpec;
use crate::{error::Error::{Cancelled, ProcessTimeout}, exec::{ExecutionRuntime, process::CommandOutput}, tools::{ToolDefinition, ToolExecutor, ToolResult}, types::ToolCall};

/// Applied when the model does not ask for a timeout.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// Hard ceiling, so a model cannot ask for an unbounded command.
const MAX_TIMEOUT: Duration = Duration::from_secs(600);

/// Runs shell commands through the execution runtime
pub struct RunCommand {
    runtime: Arc<ExecutionRuntime>,
}

impl RunCommand {
    pub fn new(runtime: Arc<ExecutionRuntime>) ->Self {
        RunCommand { runtime }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for RunCommand {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "run_command", 
            "Run a shell command in the workspace and return its output", 
            json!({
                "type":"object",
                "properties": {
                    "command": {
                        "type":"string",
                        "description": "Shell command line to run, e.g. \"cargo test\" ."
                    },
                    "cwd":{
                        "type":"string",
                        "description":"Working directory, realtive to the workspace root."
                    },
                    "timeout_secs": {
                        "type":"integer",
                        "description":"Kill the command after this many seconds (default 30, max 600)."
                    }
                },
                "required": ["command"]
            }),
        )
    }

    async fn execute(&self, call: &ToolCall) -> ToolResult {
        let args = match call.parse_arguments() {
            Ok(args) => args,
            Err(err) => {
                return  ToolResult::error(call.id.as_str(), format!("invalid arguments: {err}"));
            }
        };
        let Some(command) = args.get("command").and_then(|value| value.as_str()) else {
            return ToolResult::error(call.id.as_str(), "missing required argument: command");
        };

        let timeout = args
            .get("timeout_secs")
            .and_then(|value| value.as_u64())
            .map(Duration::from_secs)
            .unwrap_or(DEFAULT_TIMEOUT)
            .min(MAX_TIMEOUT);

        let mut spec = shell_spec(command).timeout(timeout);
        if let Some(cwd) = args.get("cwd").and_then(|value| value.as_str()) {
            spec = spec.cwd(cwd);
        }

        // The tool path has no session token yet; a bounded timeout is the guard
        // for now. Session-wide cancellation arrives with the agent loop.
        match self.runtime.run(spec, CancellationToken::new()).await {
            Ok(output) => {
                let text = render(&output);
                if output.success {
                    ToolResult::success(call.id.as_str(), text)
                } else {
                    let code = output
                        .exit_code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "signal".to_string());
                    ToolResult::error(
                        call.id.as_str(),
                        format!("command exited with {code}\n{text}"),
                    )
                }
            }
            Err(ProcessTimeout(deadline)) => ToolResult::error(
                call.id.as_str(),
                format!("command timed out after {deadline:?}"),
            ),
            Err(Cancelled) => {
                ToolResult::error(call.id.as_str(), "command was cancelled")
            }
            Err(err) => ToolResult::error(call.id.as_str(), err.to_string()),
        }
    }
}

/// Build the spec that runs a shell line.
#[cfg(unix)]
fn shell_spec(command: &str) -> CommandSpec {
    CommandSpec::new("sh").args(["-c", command])
}

#[cfg(windows)]
fn shell_spec(command: &str) -> CommandSpec {
    CommandSpec::new("cmd").args(["/C", command])
}

/// Render a completed command for the model: stdout, then stderr if present.
fn render(output: &CommandOutput) -> String {
    let mut text = String::new();

    if !output.stdout.is_empty() {
        text.push_str(&output.stdout);
    }

    if !output.stderr.is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str("[stderr]\n");
        text.push_str(&output.stderr);
    }

    if text.is_empty() {
        text.push_str("(no output)");
    }

    text
}
