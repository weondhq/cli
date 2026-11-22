#![forbid(unsafe_code)]

mod cli;
mod commands;
mod config;
mod http;
mod sanitize;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, ProjectsCommand};

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = Cli::parse();
    let api_base = http::validate_api_base_url(&args.api)?;
    let ctx = commands::Context {
        api_base,
        verbose: args.verbose,
    };

    match args.command {
        Command::Connect => commands::connect::run(&ctx),
        Command::Whoami => commands::whoami::run(&ctx),
        Command::Projects { command } => match command {
            ProjectsCommand::List => commands::projects::run_list(&ctx),
        },
        Command::Deploy { slug } => commands::deploy::run(&ctx, &slug),
        Command::Logs {
            deployment_id,
            follow,
            limit,
        } => commands::logs::run(&ctx, &deployment_id, follow, limit),
        Command::Disconnect => commands::disconnect::run(&ctx),
    }
}
