use clap::Parser;
use crate::cli::{CLI, Commands};
use caraxes_internal::{model::{self, provider::ModelRequest}, types::{Message, Role::User}};

mod cli;


#[tokio::main]
async fn main(){
    dotenvy::dotenv().ok();
    
    let cli:CLI = cli::CLI::parse();

    if let Err(err) = run(cli).await {
        eprintln!("error : {err}");
        std::process::exit(1);
    }
}

async fn run(cli:CLI) -> caraxes_internal::error::Result<()> {
    match cli.command {
        Commands::Query { query } => {
            let config = model::ModelConfig::from_env();
            let provider = model::build_provider(&config)?;

            let request = ModelRequest {
                model: config.model.clone(),
                messages: vec![Message::new(User, query)],
            };

            let response = provider.complete(request).await?;
            println!("{}", response.content.unwrap_or_default());
            Ok(())
        }
    }
}