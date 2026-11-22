use clap::{Parser, Subcommand};

pub const DEFAULT_API_BASE_URL: &str = "https://api.weond.com";

#[derive(Debug, Parser)]
#[command(
    name = "weoc",
    version,
    about = "Weond CLI — Manage deployments, projects, and logs from your terminal",
    long_about = "Official Weond CLI for authenticating via Device Code Flow, listing workspace projects, triggering deployments, and streaming deployment logs."
)]
pub struct Cli {
    /// Control plane API base URL (https:// required for remote hosts)
    #[arg(long, global = true, env = "WEOND_API_URL", default_value = DEFAULT_API_BASE_URL)]
    pub api: String,

    /// Enable verbose output
    #[arg(short, long, global = true, default_value_t = false)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Connect your Weond account using the OAuth 2.0 Device Code Flow
    Connect,

    /// Show the currently connected account
    Whoami,

    /// Manage workspace projects
    Projects {
        #[command(subcommand)]
        command: ProjectsCommand,
    },

    /// Trigger a deployment for a project
    Deploy {
        /// Project slug to deploy
        slug: String,
    },

    /// View or stream deployment logs
    Logs {
        /// Deployment ID to inspect
        deployment_id: String,

        /// Stream live logs until interrupted
        #[arg(short, long, default_value_t = false)]
        follow: bool,

        /// Maximum number of initial log entries to display (1..=500)
        #[arg(short = 'n', long, default_value_t = 100, value_parser = clap::value_parser!(u16).range(1..=500))]
        limit: u16,
    },

    /// Revoke the current CLI session and delete local credentials
    Disconnect,
}

#[derive(Debug, Subcommand)]
pub enum ProjectsCommand {
    /// List workspace projects
    List,
}
