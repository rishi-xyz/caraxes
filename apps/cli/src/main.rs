use std::io::Write;
use std::sync::Arc;

use crate::cli::{CLI, Commands};
use caraxes_internal::error::{Error, Result};
use caraxes_internal::exec::ExecutionRuntime;
use caraxes_internal::model::provider::ModelProvider;
use caraxes_internal::tools::read_file::ReadFile;
use caraxes_internal::tools::{ToolRegistry, ToolResult};
use caraxes_internal::types::ToolCall;
use caraxes_internal::{
    error::Error::Cancelled,
    model::{
        self,
        provider::{ModelRequest, StreamEvent},
    },
    types::{Message, Role::User},
};
use clap::Parser;
use futures::StreamExt;
use tokio_util::io::simplex::new;
use tokio_util::sync::CancellationToken;

mod cli;

const MAX_TURNS: usize = 8;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let cli: CLI = cli::CLI::parse();

    match run(cli).await {
        Ok(()) => {}
        Err(Cancelled) => {
            eprintln!("cancelled");
            std::process::exit(130)
        }
        Err(err) => {
            eprintln!("error : {err}");
            std::process::exit(1);
        }
    }
}

async fn run(cli: CLI) -> Result<()> {
    match cli.command {
        Commands::Query { query } => {
            let config = model::ModelConfig::from_env();
            let provider = model::build_provider(&config)?;

            let cancel = CancellationToken::new();
            ctrl_c_watcher(cancel.clone());

            let runtime = Arc::new(ExecutionRuntime::open(std::env::current_dir()?)?);

            let mut registry = ToolRegistry::new();
            registry.register(Arc::new(ReadFile::new(runtime.clone())));
            let tools = registry.definitions();

            let mut messages = vec![Message::new(User, query)];

            for _ in 0..MAX_TURNS {
                let request = ModelRequest::new(config.model.clone(), messages.clone())
                    .with_tools(tools.clone());

                let (_text, tool_calls) =
                    stream_turn(provider.as_ref(), request, cancel.clone()).await?;

                if tool_calls.is_empty() {
                    return Ok(());
                }

                messages.push(Message::assistant_tool_calls(tool_calls.clone()));

                for call in &tool_calls {
                    let result = registry.execute(call).await;
                    eprintln!(
                        "[tool] {}({}) -> {}",
                        call.name,
                        call.arguments,
                        tool_call_result(&result)
                    );
                    messages.push(result.into_message());
                }
            }

            eprintln!("stopped after {MAX_TURNS} tool turns without a final answer");
            Ok(())
        }
    }
}

/// Run one streaming model turn
async fn stream_turn(
    provider: &dyn ModelProvider,
    request: ModelRequest,
    cancel: CancellationToken,
) -> Result<(String, Vec<ToolCall>)> {
    let mut stream = provider.stream(request, cancel).await?;
    let mut text = String::new();
    let mut tool_calls = Vec::new();

    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::Token(token) => {
                println!("{token}");
                std::io::stdout().flush().ok();
                text.push_str(&token);
            }
            StreamEvent::Done {
                usage,
                finish_reason,
                tool_calls: calls,
            } => {
                println!();
                tool_calls = calls;
            }
        }
    }
    Ok((text, tool_calls))
}
fn tool_call_result(result: &ToolResult) -> String {
    if result.is_error {
        format!("error: {}", result.content)
    } else {
        format!("ok ({} chars)", result.content.len())
    }
}

fn ctrl_c_watcher(cancel: CancellationToken) {
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                cancel.cancel();
            }
            Err(_) => (),
        }
    });
}
