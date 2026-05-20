mod auth;
mod cli;
mod client;
mod commands;
mod config;
mod encoding;
mod env;
mod http;
mod output;
mod pipeline;
mod release_migration;
mod repo;
mod update;

use anyhow::Context;
use clap::{
    Parser,
    error::ErrorKind::{DisplayHelp, DisplayVersion},
};

use crate::{
    auth::KeyringCredentialStore,
    cli::{Cli, ParseDiagnostic},
    config::Config,
};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("gd: {error:#}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    let cli = match Cli::try_parse_from(args.clone()) {
        Ok(cli) => cli,
        Err(error) if matches!(error.kind(), DisplayHelp | DisplayVersion) => error.exit(),
        Err(error) => {
            let diagnostic = ParseDiagnostic::from_error(&args, &error);
            eprintln!("{}", diagnostic.render_stderr());
            std::process::exit(error.exit_code());
        }
    };
    let mut config = Config::load().await.context("failed to load gd config")?;
    let api_base = cli
        .global
        .api_base
        .as_deref()
        .map(str::to_string)
        .or_else(|| env::gitcode_api_base_env().map(|(_, value)| value));
    config.apply_overrides(Some(cli.global.hostname.as_str()), api_base.as_deref())?;

    let credentials = KeyringCredentialStore::new();
    commands::run(cli, config, &credentials).await
}
