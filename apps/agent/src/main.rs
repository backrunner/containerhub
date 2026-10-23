mod cli;
mod commands;
use clap::Parser;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    commands::run(cli::Cli::parse().command).await
}
