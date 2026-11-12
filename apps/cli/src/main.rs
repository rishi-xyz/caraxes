use std::{env, process};
use clap::Parser;
use crate::cli::{CLI, Commands};
use caraxes_internal::model;

mod cli;


#[tokio::main]
async fn main(){
    dotenvy::dotenv().ok();
    
    let cli:CLI = cli::CLI::parse();

    match cli.command {
        Commands::Query { query } => match model::llm_call(&query).await {
            Ok(response) => println!("{response}"),
            Err(err) => {
                eprintln!("error: {err}");
                process::exit(1);
            }
        },
    }
}