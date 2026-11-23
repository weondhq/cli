use crate::commands::{Context, resolve_project};
use crate::config::load_token;
use crate::http::graphql_request;
use crate::sanitize::sanitize_terminal_text;
use anyhow::{Context as _, Result, bail};
use serde_json::json;

pub fn run_list(ctx: &Context, project_slug: Option<&str>) -> Result<()> {
    let token = load_token().context("connect your account first with 'weoc connect'")?;
    let project = resolve_project(ctx, &token.access_token, project_slug)?;

    let data = graphql_request(
        &ctx.api_base,
        &token.access_token,
        "WorkspaceCatalog",
        "query WorkspaceCatalog($workspaceId: UUID) { catalog(workspaceId: $workspaceId) { projects { id env { key updatedAt } } } }",
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

    if envs.is_empty() {
        println!(
            "No environment variables configured for '{}'.",
            sanitize_terminal_text(&project.slug)
        );
        return Ok(());
    }

    println!(
        "Environment variables for '{}' (values are encrypted at rest):",
        sanitize_terminal_text(&project.slug)
    );
    for item in envs {
        let key = item.get("key").and_then(|v| v.as_str()).unwrap_or("");
        let updated = item.get("updatedAt").and_then(|v| v.as_str()).unwrap_or("—");
        println!(
            "  {:<32}  (updated {})",
            sanitize_terminal_text(key),
            sanitize_terminal_text(updated)
        );
    }
    Ok(())
}

pub fn run_set(ctx: &Context, key: &str, value: &str, project_slug: Option<&str>) -> Result<()> {
    let clean_key = key.trim();
    if clean_key.is_empty() {
        bail!("environment variable key cannot be empty");
    }
    let token = load_token().context("connect your account first with 'weoc connect'")?;
    let project = resolve_project(ctx, &token.access_token, project_slug)?;

    graphql_request(
        &ctx.api_base,
        &token.access_token,
        "SetEnvVar",
        "mutation SetEnvVar($projectId: UUID!, $key: String!, $value: String!) { setEnvVar(projectId: $projectId, key: $key, value: $value) { key } }",
        json!({
            "projectId": project.id,
            "key": clean_key,
            "value": value,
        }),
    )?;

    println!(
        "Saved environment variable '{}' on project '{}'.",
        sanitize_terminal_text(clean_key),
        sanitize_terminal_text(&project.slug)
    );
    Ok(())
}

pub fn run_rm(ctx: &Context, key: &str, project_slug: Option<&str>) -> Result<()> {
    let clean_key = key.trim();
    if clean_key.is_empty() {
        bail!("environment variable key cannot be empty");
    }
    let token = load_token().context("connect your account first with 'weoc connect'")?;
    let project = resolve_project(ctx, &token.access_token, project_slug)?;

    graphql_request(
        &ctx.api_base,
        &token.access_token,
        "DeleteEnvVar",
        "mutation DeleteEnvVar($projectId: UUID!, $key: String!) { deleteEnvVar(projectId: $projectId, key: $key) { deleted } }",
        json!({
            "projectId": project.id,
            "key": clean_key,
        }),
    )?;

    println!(
        "Removed environment variable '{}' from project '{}'.",
        sanitize_terminal_text(clean_key),
        sanitize_terminal_text(&project.slug)
    );
    Ok(())
}
