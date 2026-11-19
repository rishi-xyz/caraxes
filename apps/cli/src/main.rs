use std::io::Write;

use crate::cli::{CLI, Commands};
use caraxes_internal::error::Error;
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
use tokio_util::sync::CancellationToken;

mod cli;

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

async fn run(cli: CLI) -> caraxes_internal::error::Result<()> {
    match cli.command {
        Commands::Query { query } => {
            let config = model::ModelConfig::from_env();
            let provider = model::build_provider(&config)?;

            let cancel = CancellationToken::new();
            ctrl_c_watcher(cancel.clone());

            let request = ModelRequest::new(config.model.clone(), vec![Message::new(User, query)]);

            let mut stream = provider.stream(request, cancel).await?;

            while let Some(event) = stream.next().await {
                match event? {
                    StreamEvent::Token(token) => {
                        print!("{token}");
                        std::io::stdout().flush().ok();
                    }
                    StreamEvent::Done {
                        usage,
                        finish_reason,
                        tool_calls,
                    } => {
                        for call in &tool_calls {
                            eprintln!("[tool call] {} ({})", call.name, call.arguments);
                        }
                        println!("");
                    }
                }
            }
            Ok(())
        }
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
