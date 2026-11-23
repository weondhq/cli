use crate::cli::EnvTarget;
use crate::commands::{Context, resolve_project};
use crate::http::graphql_request;
use crate::sanitize::sanitize_terminal_text;
use anyhow::{Result, bail};
use serde_json::json;

pub fn run_list(
    ctx: &Context,
    project_slug: Option<&str>,
    env_filter: Option<EnvTarget>,
) -> Result<()> {
    let auth = ctx.resolve_auth()?;
    let project = resolve_project(ctx, auth.access_token.as_str(), project_slug)?;

    let data = graphql_request(
        &ctx.api_base,
        auth.access_token.as_str(),
        ctx.verbose,
        "WorkspaceCatalog",
        "query WorkspaceCatalog($workspaceId: UUID) { catalog(workspaceId: $workspaceId) { projects { id env { id key environments } } } }",
        json!({ "workspaceId": project.workspace_id }),
    )?;

    let empty = Vec::new();
    let projects = data
        .get("catalog")
        .and_then(|c| c.get("projects"))
        .and_then(|p| p.as_array())
        .unwrap_or(&empty);

    let envs = projects
        .iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(project.id.as_str()))
        .and_then(|p| p.get("env"))
        .and_then(|e| e.as_array())
        .unwrap_or(&empty);

    let filtered: Vec<_> = envs
        .iter()
        .filter(|item| match env_filter {
            None => true,
            Some(target) => item
                .get("environments")
                .and_then(|v| v.as_array())
                .is_some_and(|arr| arr.iter().any(|e| e.as_str() == Some(target.as_str()))),
        })
        .collect();

    if filtered.is_empty() {
        match env_filter {
            Some(target) => println!(
                "No environment variables configured for '{}' in '{}'.",
                sanitize_terminal_text(&project.slug),
                target.as_str()
            ),
            None => println!(
                "No environment variables configured for '{}'.",
                sanitize_terminal_text(&project.slug)
            ),
        }
        return Ok(());
    }

    println!(
        "Environment variables for '{}' (values are encrypted at rest):",
        sanitize_terminal_text(&project.slug)
    );
    for item in filtered {
        let key = item.get("key").and_then(|v| v.as_str()).unwrap_or("");
        let env_list: Vec<String> = item
            .get("environments")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| e.as_str())
                    .map(sanitize_terminal_text)
                    .collect()
            })
            .unwrap_or_default();
        let env_display = if env_list.is_empty() {
            "all".to_string()
        } else {
            env_list.join(", ")
        };
        println!("  {:<32}  [{}]", sanitize_terminal_text(key), env_display);
    }
    Ok(())
}

pub fn run_set(
    ctx: &Context,
    key: &str,
    value: &str,
    project_slug: Option<&str>,
    env_target: Option<EnvTarget>,
) -> Result<()> {
    let clean_key = key.trim();
    if clean_key.is_empty() {
        bail!("environment variable key cannot be empty");
    }
    let auth = ctx.resolve_auth()?;
    let project = resolve_project(ctx, auth.access_token.as_str(), project_slug)?;

    let (query, variables) = match env_target {
        Some(target) => (
            "mutation SetEnvVar($projectId: UUID!, $key: String!, $value: String!, $environments: [String!]) { setEnvVar(projectId: $projectId, key: $key, value: $value, environments: $environments) { key } }",
            json!({
                "projectId": project.id,
                "key": clean_key,
                "value": value,
                "environments": [target.as_str()],
            }),
        ),
        None => (
            "mutation SetEnvVar($projectId: UUID!, $key: String!, $value: String!) { setEnvVar(projectId: $projectId, key: $key, value: $value) { key } }",
            json!({
                "projectId": project.id,
                "key": clean_key,
                "value": value,
            }),
        ),
    };

    graphql_request(
        &ctx.api_base,
        auth.access_token.as_str(),
        ctx.verbose,
        "SetEnvVar",
        query,
        variables,
    )?;

    match env_target {
        Some(target) => println!(
            "Saved environment variable '{}' on project '{}' ({}).",
            sanitize_terminal_text(clean_key),
            sanitize_terminal_text(&project.slug),
            target.as_str()
        ),
        None => println!(
            "Saved environment variable '{}' on project '{}'.",
            sanitize_terminal_text(clean_key),
            sanitize_terminal_text(&project.slug)
        ),
    }
    Ok(())
}

pub fn run_rm(
    ctx: &Context,
    key: &str,
    project_slug: Option<&str>,
    env_target: Option<EnvTarget>,
) -> Result<()> {
    let clean_key = key.trim();
    if clean_key.is_empty() {
        bail!("environment variable key cannot be empty");
    }
    let auth = ctx.resolve_auth()?;
    let project = resolve_project(ctx, auth.access_token.as_str(), project_slug)?;

    let (query, variables) = match env_target {
        Some(target) => (
            "mutation DeleteEnvVar($projectId: UUID!, $key: String!, $environments: [String!]) { deleteEnvVar(projectId: $projectId, key: $key, environments: $environments) { deleted } }",
            json!({
                "projectId": project.id,
                "key": clean_key,
                "environments": [target.as_str()],
            }),
        ),
        None => (
            "mutation DeleteEnvVar($projectId: UUID!, $key: String!) { deleteEnvVar(projectId: $projectId, key: $key) { deleted } }",
            json!({
                "projectId": project.id,
                "key": clean_key,
            }),
        ),
    };

    graphql_request(
        &ctx.api_base,
        auth.access_token.as_str(),
        ctx.verbose,
        "DeleteEnvVar",
        query,
        variables,
    )?;

    match env_target {
        Some(target) => println!(
            "Removed environment variable '{}' from project '{}' ({}).",
            sanitize_terminal_text(clean_key),
            sanitize_terminal_text(&project.slug),
            target.as_str()
        ),
        None => println!(
            "Removed environment variable '{}' from project '{}'.",
            sanitize_terminal_text(clean_key),
            sanitize_terminal_text(&project.slug)
        ),
    }
    Ok(())
}
