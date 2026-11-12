use clap::{Command, Parser, Subcommand};


/// # CLI
/// 
/// main caraxes cli struct
#[derive(Parser)]
#[command(name = "caraxes")]
#[command(about = "A Local first AI-native development enviroment and agent runtime", long_about=None)]
pub struct CLI {
    #[command(subcommand)]
    pub command: Commands
}

/// ### Commands for caraxes-cli
#[derive(Debug, Subcommand)]
pub enum Commands{
    /// Send a one-shot prompt to the configurted model and print the answer.
    Query { query: String }
}