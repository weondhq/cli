pub mod connect;
pub mod deploy;
pub mod disconnect;
pub mod domains;
pub mod env;
pub mod link;
pub mod logs;
pub mod projects;
pub mod rollback;
pub mod whoami;

use crate::http::{build_client, read_bounded_body};
use anyhow::{Context as _, Result, bail};
use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Context {
    pub api_base: String,
    pub verbose: bool,
}

#[derive(Debug, Deserialize)]
struct ProjectsEnvelope {
    data: ProjectsListData,
}

#[derive(Debug, Deserialize)]
struct ProjectsListData {
    #[serde(default)]
    projects: Vec<ResolvedProject>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedProject {
    pub id: String,
    pub slug: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub workspace_id: String,
}

/// Resolves a project slug (explicit argument or linked `.weond.json`) to its full project record.
pub fn resolve_project(
    ctx: &Context,
    access_token: &str,
    explicit_slug: Option<&str>,
) -> Result<ResolvedProject> {
    let target_slug = match explicit_slug.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => s.to_string(),
        None => link::load_linked_slug()?
            .context("no project specified; pass a slug or run 'weoc link <slug>' first")?,
    };

    let endpoint = format!("{}/api/projects", ctx.api_base);
    let client = build_client(Duration::from_secs(20))?;
    let resp = client
        .get(&endpoint)
        .bearer_auth(access_token)
        .send()
        .context("failed to list projects")?;

    let status = resp.status();
    let body = read_bounded_body(resp)?;
    if status == StatusCode::UNAUTHORIZED {
        bail!("session token is invalid or expired; run 'weoc connect' again");
    }
    if status != StatusCode::OK {
        bail!("failed to load projects (HTTP {})", status.as_u16());
    }

    let parsed: ProjectsEnvelope =
        serde_json::from_slice(&body).context("failed to decode projects list")?;
    parsed
        .data
        .projects
        .into_iter()
        .find(|p| p.slug.eq_ignore_ascii_case(&target_slug))
        .with_context(|| format!("project '{target_slug}' not found in your workspaces"))
}
