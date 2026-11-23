#![forbid(unsafe_code)]

mod cli;
mod commands;
mod config;
mod http;
mod sanitize;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, DomainsCommand, EnvCommand, ProjectsCommand};

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
        Command::Link { slug } => commands::link::run(&ctx, &slug),
        Command::Deploy { slug } => commands::deploy::run(&ctx, slug.as_deref()),
        Command::Rollback {
            deployment_id,
            project,
        } => commands::rollback::run(&ctx, &deployment_id, project.as_deref()),
        Command::Env { command } => match command {
            EnvCommand::List { project } => commands::env::run_list(&ctx, project.as_deref()),
            EnvCommand::Set {
                key,
                value,
                project,
            } => commands::env::run_set(&ctx, &key, &value, project.as_deref()),
            EnvCommand::Rm { key, project } => {
                commands::env::run_rm(&ctx, &key, project.as_deref())
            }
        },
        Command::Domains { command } => match command {
            DomainsCommand::List { project } => {
                commands::domains::run_list(&ctx, project.as_deref())
            }
            DomainsCommand::Add { hostname, project } => {
                commands::domains::run_add(&ctx, &hostname, project.as_deref())
            }
            DomainsCommand::Verify { domain_id } => commands::domains::run_verify(&ctx, &domain_id),
        },
        Command::Logs {
            deployment_id,
            follow,
            limit,
        } => commands::logs::run(&ctx, &deployment_id, follow, limit),
        Command::Disconnect => commands::disconnect::run(&ctx),
    }
}
