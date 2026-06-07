use anyhow::Result;
use clap::Parser;

mod admin;
mod args;
mod commands;
mod connection;
mod output;
mod parsers;

use args::Cli;
use commands::run_command;
use connection::connect;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let connection = connect(&cli).await?;

    run_command(
        &cli,
        &connection.pool,
        &connection.utils,
        &connection.schema,
    )
    .await
}
