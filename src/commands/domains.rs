use crate::commands::{Context, resolve_project};
use crate::http::graphql_request;
use crate::sanitize::sanitize_terminal_text;
use anyhow::{Context as _, Result, bail};
use serde_json::json;

pub fn run_list(ctx: &Context, project_slug: Option<&str>) -> Result<()> {
    let auth = ctx.resolve_auth()?;
    let project = resolve_project(ctx, auth.access_token.as_str(), project_slug)?;

    let data = graphql_request(
        &ctx.api_base,
        auth.access_token.as_str(),
        ctx.verbose,
        "WorkspaceCatalog",
        "query WorkspaceCatalog($workspaceId: UUID) { catalog(workspaceId: $workspaceId) { domains { id projectId hostname status sslStatus } } }",
        json!({ "workspaceId": project.workspace_id }),
    )?;

    let empty = Vec::new();
    let domains = data
        .get("catalog")
        .and_then(|c| c.get("domains"))
        .and_then(|d| d.as_array())
        .unwrap_or(&empty);

    let matched: Vec<_> = domains
        .iter()
        .filter(|d| d.get("projectId").and_then(|v| v.as_str()) == Some(project.id.as_str()))
        .collect();

    if matched.is_empty() {
        println!(
            "No custom domains attached to '{}'.",
            sanitize_terminal_text(&project.slug)
        );
        return Ok(());
    }

    println!(
        "Custom domains for '{}':",
        sanitize_terminal_text(&project.slug)
    );
    for d in matched {
        let id = d.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let host = d.get("hostname").and_then(|v| v.as_str()).unwrap_or("");
        let status = d
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("pending");
        let ssl = d
            .get("sslStatus")
            .and_then(|v| v.as_str())
            .unwrap_or("pending");
        println!(
            "  {:<28}  status={:<10} ssl={:<18} id={}",
            sanitize_terminal_text(host),
            sanitize_terminal_text(status),
            sanitize_terminal_text(ssl),
            sanitize_terminal_text(id),
        );
    }
    Ok(())
}

pub fn run_add(ctx: &Context, hostname: &str, project_slug: Option<&str>) -> Result<()> {
    let clean_host = hostname.trim().to_ascii_lowercase();
    if clean_host.is_empty() {
        bail!("hostname cannot be empty");
    }
    let auth = ctx.resolve_auth()?;
    let project = resolve_project(ctx, auth.access_token.as_str(), project_slug)?;

    let data = graphql_request(
        &ctx.api_base,
        auth.access_token.as_str(),
        ctx.verbose,
        "AddDomain",
        "mutation AddDomain($workspaceId: UUID!, $projectId: UUID, $hostname: String!, $environment: String) { addDomain(workspaceId: $workspaceId, projectId: $projectId, hostname: $hostname, environment: $environment) { id hostname status cname { name value } verification { type name value } } }",
        json!({
            "workspaceId": project.workspace_id,
            "projectId": project.id,
            "hostname": clean_host,
            "environment": "production",
        }),
    )?;

    let added = data
        .get("addDomain")
        .context("missing addDomain response")?;
    let id = added.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let cname = added
        .get("cname")
        .and_then(|c| c.get("value"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("edge.weond.app");
    let vname = added
        .get("verification")
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let vval = added
        .get("verification")
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    println!("Added domain '{}'.", sanitize_terminal_text(&clean_host));
    println!("  Domain ID: {}", sanitize_terminal_text(id));
    println!("  CNAME/ALIAS target: {}", sanitize_terminal_text(cname));
    if !vname.is_empty() && !vval.is_empty() {
        println!(
            "  TXT verification:   {} -> {}",
            sanitize_terminal_text(vname),
            sanitize_terminal_text(vval)
        );
    }
    println!(
        "\nVerify once DNS propagates: weoc domains verify {}",
        sanitize_terminal_text(id)
    );
    Ok(())
}

pub fn run_verify(ctx: &Context, domain_id: &str) -> Result<()> {
    let clean_id = domain_id.trim();
    if clean_id.is_empty() {
        bail!("domain ID cannot be empty");
    }
    let auth = ctx.resolve_auth()?;

    let data = graphql_request(
        &ctx.api_base,
        auth.access_token.as_str(),
        ctx.verbose,
        "VerifyDomain",
        "mutation VerifyDomain($id: UUID!) { verifyDomain(id: $id) { hostname status sslStatus lastError } }",
        json!({ "id": clean_id }),
    )?;

    let verified = data
        .get("verifyDomain")
        .context("missing verifyDomain response")?;
    let host = verified
        .get("hostname")
        .and_then(|v| v.as_str())
        .unwrap_or(clean_id);
    let status = verified
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("pending");
    let ssl = verified
        .get("sslStatus")
        .and_then(|v| v.as_str())
        .unwrap_or("pending");
    let last_err = verified
        .get("lastError")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    println!(
        "Domain verification result for '{}':",
        sanitize_terminal_text(host)
    );
    println!("  Status: {}", sanitize_terminal_text(status));
    println!("  SSL:    {}", sanitize_terminal_text(ssl));
    if !last_err.is_empty() {
        println!("  Note:   {}", sanitize_terminal_text(last_err));
    }
    Ok(())
}
