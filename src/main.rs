mod auth;
mod cli;
mod client;
mod commands;
mod config;
mod output;
mod repo;

use anyhow::Context;
use clap::Parser;

use crate::{auth::KeyringCredentialStore, cli::Cli, config::Config};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("gd: {error:#}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut config = Config::load().context("failed to load gd config")?;
    config.apply_overrides(
        Some(cli.global.hostname.as_str()),
        Some(cli.global.api_base.as_str()),
    )?;

    let credentials = KeyringCredentialStore::new();
    commands::run(cli, config, &credentials).await
}
