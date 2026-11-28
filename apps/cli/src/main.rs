use std::io::Write;
use std::sync::Arc;

use crate::cli::{CLI, Commands};
use caraxes_internal::error::{Error::Cancelled, Result};
use caraxes_internal::exec::ExecutionRuntime;
use caraxes_internal::model::provider::ModelProvider;
use caraxes_internal::tools::{
    ToolRegistry, 
    ToolResult,
    read_file::ReadFile,
    run_command::RunCommand
};
use caraxes_internal::types::ToolCall;
use caraxes_internal::{
    model::{
        self,
        provider::{ModelRequest, StreamEvent},
    },
    types::{Message, Role::User},
};
use clap::Parser;
use futures::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, trace, warn};

mod cli;

const MAX_TURNS: usize = 8;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    init_logs();

    let cli: CLI = cli::CLI::parse();

    match run(cli).await {
        Ok(()) => {}
        Err(Cancelled) => {
            warn!("cancelled");
            std::process::exit(130)
        }
        Err(err) => {
            error!(error = %err, "run failed");
            std::process::exit(1);
        }
    }
}

/// Env-controlled filtering (`CARAXES_LOG`, then `RUST_LOG`, else off).
#[cfg(feature = "dev-logs")]
fn init_logs() {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{EnvFilter, fmt};

    let filter = ["CARAXES_LOG", "RUST_LOG"]
        .iter()
        .find_map(|name| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "off".to_string());

    tracing_subscriber::registry()
        .with(EnvFilter::try_new(filter).unwrap_or_else(|_| EnvFilter::new("off")))
        .with(
            fmt::layer()
                .with_writer(std::io::stderr)
                .with_span_events(fmt::format::FmtSpan::CLOSE),
        )
        .init();
}

/// Fixed info-level output for lean builds (`--no-default-features`).
#[cfg(not(feature = "dev-logs"))]
fn init_logs() {
    use tracing::level_filters::LevelFilter;

    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::INFO)
        .with_writer(std::io::stderr)
        .init();
}

#[instrument(skip_all)]
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
            registry.register(Arc::new(RunCommand::new(runtime.clone())));
            let tools = registry.definitions();

            let mut messages = vec![Message::new(User, query)];

            for turn in 1..=MAX_TURNS {
                debug!(
                    turn,
                    model = %config.model,
                    messages = messages.len(),
                    tools = tools.len(),
                    "starting model turn"
                );

                let request = ModelRequest::new(config.model.clone(), messages.clone())
                    .with_tools(tools.clone());

                let (_text, tool_calls) =
                    stream_turn(provider.as_ref(), request, cancel.clone()).await?;

                if tool_calls.is_empty() {
                    info!(turn, "final answer received");
                    return Ok(());
                }

                messages.push(Message::assistant_tool_calls(tool_calls.clone()));

                for call in &tool_calls {
                    let result = registry.execute(call).await;
                    debug!(tool = %call.name, arguments = %call.arguments, "tool call arguments");
                    info!(
                        tool = %call.name,
                        success = !result.is_error,
                        result = %tool_call_result(&result),
                        "tool executed"
                    );
                    messages.push(result.into_message());
                }
            }

            warn!(turns = MAX_TURNS, "stopped without a final answer");
            Ok(())
        }
    }
}

/// Run one streaming model turn
#[instrument(skip_all)]
async fn stream_turn(
    provider: &dyn ModelProvider,
    request: ModelRequest,
    cancel: CancellationToken,
) -> Result<(String, Vec<ToolCall>)> {
    debug!(
        model = %request.model,
        messages = request.messages.len(),
        tools = request.tools.len(),
        "opening model stream"
    );

    let mut stream = provider.stream(request, cancel).await?;
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    let mut tokens = 0usize;
    let mut saw_done = false;
    let mut stream_error = None;

    while let Some(event) = stream.next().await {
        let event = match event {
            Ok(event) => event,
            Err(err) => {
                stream_error = Some(err);
                break;
            }
        };

        match event {
            StreamEvent::Token(token) => {
                print!("{token}");
                std::io::stdout().flush().ok();
                text.push_str(&token);
                tokens += 1;
                trace!(chars = token.len(), "streamed token");
            }
            StreamEvent::Done {
                usage,
                finish_reason,
                tool_calls: calls,
            } => {
                println!();
                saw_done = true;
                debug!(
                    ?usage,
                    ?finish_reason,
                    tokens,
                    chars = text.len(),
                    tool_calls = calls.len(),
                    "stream finished"
                );
                tool_calls = calls;
            }
        }
    }

    // Keep the answer on stdout clean even if the stream ended without a Done event.
    if tokens > 0 && !saw_done {
        println!();
    }

    if let Some(err) = stream_error {
        return Err(err);
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
    debug!("ctrl-c watcher started");
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                debug!("ctrl-c received; cancelling");
                cancel.cancel();
            }
            Err(err) => warn!(error = %err, "ctrl-c watcher failed"),
        }
    });
}
