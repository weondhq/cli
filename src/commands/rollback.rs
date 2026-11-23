use crate::commands::{Context, resolve_project};
use crate::config::load_token;
use crate::http::graphql_request;
use crate::sanitize::sanitize_terminal_text;
use anyhow::{Context as _, Result, bail};
use serde_json::json;

pub fn run(ctx: &Context, deployment_id: &str, project_slug: Option<&str>) -> Result<()> {
    let clean_dep = deployment_id.trim();
    if clean_dep.is_empty() {
        bail!("deployment ID cannot be empty");
    }

    let token = load_token().context("connect your account first with 'weoc connect'")?;
    let project = resolve_project(ctx, &token.access_token, project_slug)?;

    let data = graphql_request(
        &ctx.api_base,
        &token.access_token,
        "RollbackDeployment",
        "mutation RollbackDeployment($projectId: UUID!, $deploymentId: UUID!) { rollbackDeployment(projectId: $projectId, deploymentId: $deploymentId) { activeDeploymentId } }",
        json!({
            "projectId": project.id,
            "deploymentId": clean_dep,
        }),
    )?;

    let active_id = data
        .get("rollbackDeployment")
        .and_then(|r| r.get("activeDeploymentId"))
        .and_then(|v| v.as_str())
        .unwrap_or(clean_dep);

    println!(
        "Promoted deployment {} on project '{}' to production.",
        sanitize_terminal_text(active_id),
        sanitize_terminal_text(&project.slug)
    );
    Ok(())
}
