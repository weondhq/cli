#![forbid(unsafe_code)]

mod cli;
mod commands;
mod config;
mod http;
mod sanitize;

use anyhow::Result;
use clap::{CommandFactory, FromArgMatches, parser::ValueSource};
use cli::{Cli, Command, DomainsCommand, EnvCommand, ProjectsCommand};
use config::TokenSource;

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let matches = Cli::command().get_matches();
    let token_source_hint = match matches.value_source("token") {
        Some(ValueSource::CommandLine) => Some(TokenSource::CliFlag),
        Some(ValueSource::EnvVariable) => Some(TokenSource::EnvVar),
        _ => None,
    };
    let args = Cli::from_arg_matches(&matches)?;
    let api_base = http::validate_api_base_url(&args.api)?;
    let ctx = commands::Context {
        api_base,
        verbose: args.verbose,
        token_override: args.token,
        token_source_hint,
    };

    match args.command {
        Command::Connect { token_stdin } => commands::connect::run(&ctx, token_stdin),
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
            EnvCommand::List { project, env } => {
                commands::env::run_list(&ctx, project.as_deref(), env)
            }
            EnvCommand::Set {
                key,
                value,
                project,
                env,
            } => commands::env::run_set(&ctx, &key, &value, project.as_deref(), env),
            EnvCommand::Rm { key, project, env } => {
                commands::env::run_rm(&ctx, &key, project.as_deref(), env)
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
