use clap::{Parser, Subcommand, ValueEnum};

pub const DEFAULT_API_BASE_URL: &str = "https://api.weond.com";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum EnvTarget {
    Production,
    Preview,
    Development,
}

impl EnvTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Preview => "preview",
            Self::Development => "development",
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "weoc",
    version,
    about = "Weond CLI — Manage deployments, projects, and logs from your terminal",
    long_about = "Official Weond CLI for authenticating via Device Code Flow or Personal Access Tokens (WEOND_TOKEN / --token), linking local directories, listing projects, triggering or rolling back deployments, managing environment variables and custom domains, and streaming deployment logs."
)]
pub struct Cli {
    /// Control plane API base URL (https:// required for remote hosts)
    #[arg(long, global = true, env = "WEOND_API_URL", default_value = DEFAULT_API_BASE_URL)]
    pub api: String,

    /// Personal Access Token or API bearer token (overrides stored credentials without saving to disk)
    #[arg(long, global = true, env = "WEOND_TOKEN", hide_env_values = true)]
    pub token: Option<String>,

    /// Enable verbose diagnostic output
    #[arg(short, long, global = true, default_value_t = false)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Connect your Weond account via Device Code Flow, --token, or --token-stdin
    Connect {
        /// Read Personal Access Token from standard input and save after validation
        #[arg(long, default_value_t = false)]
        token_stdin: bool,
    },

    /// Show the currently connected account
    Whoami,

    /// Manage workspace projects
    Projects {
        #[command(subcommand)]
        command: ProjectsCommand,
    },

    /// Link the current directory to a Weond project (.weond.json)
    Link {
        /// Project slug to link
        slug: String,
    },

    /// Trigger a deployment for a project (uses linked project if slug is omitted)
    Deploy {
        /// Project slug to deploy (defaults to linked project in .weond.json)
        slug: Option<String>,
    },

    /// Roll back or promote a completed deployment to production
    Rollback {
        /// Completed deployment ID to promote
        deployment_id: String,

        /// Project slug (defaults to linked project in .weond.json)
        #[arg(short, long)]
        project: Option<String>,
    },

    /// Manage project environment variables
    Env {
        #[command(subcommand)]
        command: EnvCommand,
    },

    /// Manage custom domains
    Domains {
        #[command(subcommand)]
        command: DomainsCommand,
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

#[derive(Debug, Subcommand)]
pub enum EnvCommand {
    /// List environment variable keys for a project
    List {
        /// Project slug (defaults to linked project in .weond.json)
        #[arg(short, long)]
        project: Option<String>,
        /// Filter by target environment (production, preview, development)
        #[arg(short = 'e', long = "env", value_enum)]
        env: Option<EnvTarget>,
    },
    /// Set or update an environment variable
    Set {
        /// Environment variable key (e.g. API_URL)
        key: String,
        /// Environment variable value
        value: String,
        /// Project slug (defaults to linked project in .weond.json)
        #[arg(short, long)]
        project: Option<String>,
        /// Target environment (production, preview, development)
        #[arg(short = 'e', long = "env", value_enum)]
        env: Option<EnvTarget>,
    },
    /// Remove an environment variable
    Rm {
        /// Environment variable key to delete
        key: String,
        /// Project slug (defaults to linked project in .weond.json)
        #[arg(short, long)]
        project: Option<String>,
        /// Target environment (production, preview, development)
        #[arg(short = 'e', long = "env", value_enum)]
        env: Option<EnvTarget>,
    },
}

#[derive(Debug, Subcommand)]
pub enum DomainsCommand {
    /// List domains for a project
    List {
        /// Project slug (defaults to linked project in .weond.json)
        #[arg(short, long)]
        project: Option<String>,
    },
    /// Attach a custom domain to a project
    Add {
        /// Hostname to attach (e.g. example.com or app.example.com)
        hostname: String,
        /// Project slug (defaults to linked project in .weond.json)
        #[arg(short, long)]
        project: Option<String>,
    },
    /// Trigger DNS and TLS verification for a domain ID
    Verify {
        /// Domain ID returned by 'weoc domains list' or 'weoc domains add'
        domain_id: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn clap_command_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_global_token_and_connect_token_stdin() {
        let cli = Cli::try_parse_from(["weoc", "--token", "pat_abc", "whoami"]).unwrap();
        assert_eq!(cli.token.as_deref(), Some("pat_abc"));
        assert!(matches!(cli.command, Command::Whoami));

        let connect_tok =
            Cli::try_parse_from(["weoc", "connect", "--token", "pat_save_123"]).unwrap();
        assert_eq!(connect_tok.token.as_deref(), Some("pat_save_123"));
        assert!(matches!(
            connect_tok.command,
            Command::Connect { token_stdin: false }
        ));

        let connect_stdin = Cli::try_parse_from(["weoc", "connect", "--token-stdin"]).unwrap();
        assert!(matches!(
            connect_stdin.command,
            Command::Connect { token_stdin: true }
        ));
    }

    #[test]
    fn parses_env_target_and_validates_logs_limit_bounds() {
        let env_set = Cli::try_parse_from([
            "weoc", "env", "set", "API_KEY", "val", "-p", "my-app", "-e", "preview",
        ])
        .unwrap();
        match env_set.command {
            Command::Env {
                command:
                    EnvCommand::Set {
                        key,
                        value,
                        project,
                        env,
                    },
            } => {
                assert_eq!(key, "API_KEY");
                assert_eq!(value, "val");
                assert_eq!(project.as_deref(), Some("my-app"));
                assert_eq!(env, Some(EnvTarget::Preview));
            }
            other => panic!("unexpected parsed command: {other:?}"),
        }

        assert!(Cli::try_parse_from(["weoc", "logs", "dep-1", "-n", "1"]).is_ok());
        assert!(Cli::try_parse_from(["weoc", "logs", "dep-1", "-n", "500"]).is_ok());
        assert!(Cli::try_parse_from(["weoc", "logs", "dep-1", "-n", "0"]).is_err());
        assert!(Cli::try_parse_from(["weoc", "logs", "dep-1", "-n", "501"]).is_err());
    }
}
