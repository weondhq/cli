use crate::commands::{Context, resolve_project};
use crate::http::graphql_request;
use crate::sanitize::sanitize_terminal_text;
use anyhow::{Result, bail};
use serde_json::json;

pub fn run(ctx: &Context, deployment_id: &str, project_slug: Option<&str>) -> Result<()> {
    let clean_dep = deployment_id.trim();
    if clean_dep.is_empty() {
        bail!("deployment ID cannot be empty");
    }

    let auth = ctx.resolve_auth()?;
    let project = resolve_project(ctx, auth.access_token.as_str(), project_slug)?;

    let data = graphql_request(
        &ctx.api_base,
        auth.access_token.as_str(),
        ctx.verbose,
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
