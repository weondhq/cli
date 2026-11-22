use crate::commands::Context;
use crate::config::load_token;
use crate::http::{build_client, read_bounded_body};
use crate::sanitize::sanitize_terminal_text;
use anyhow::{Context as _, Result, bail};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct DeployEnvelope {
    data: DeployData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeployData {
    #[serde(default)]
    project: DeployProject,
    #[serde(default)]
    deployment_id: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeployProject {
    #[serde(default)]
    preview_url: String,
}

pub fn run(ctx: &Context, slug: &str) -> Result<()> {
    let trimmed_slug = slug.trim();
    if trimmed_slug.is_empty() {
        bail!("project slug cannot be empty");
    }

    let token = load_token().context("connect your account first with 'weoc connect'")?;
    let encoded_slug = utf8_percent_encode(trimmed_slug, NON_ALPHANUMERIC).to_string();
    let endpoint = format!("{}/api/projects/{encoded_slug}/deploy", ctx.api_base);

    let client = build_client(Duration::from_secs(30))?;
    let resp = client
        .post(&endpoint)
        .bearer_auth(&token.access_token)
        .send()
        .context("failed to trigger deployment")?;

    let status = resp.status();
    let body = read_bounded_body(resp)?;

    if status == StatusCode::UNAUTHORIZED {
        bail!("session token is invalid or expired; run 'weoc connect' again");
    }
    if status != StatusCode::OK && status != StatusCode::CREATED {
        bail!(
            "deployment failed (HTTP {}): {}",
            status.as_u16(),
            sanitize_terminal_text(&String::from_utf8_lossy(&body))
        );
    }

    let result: DeployEnvelope =
        serde_json::from_slice(&body).context("failed to decode deployment response")?;

    let dep_id = sanitize_terminal_text(&result.data.deployment_id);
    let preview = sanitize_terminal_text(&result.data.project.preview_url);

    println!("Deployment triggered.");
    println!("  Project:       {}", sanitize_terminal_text(trimmed_slug));
    println!("  Deployment ID: {dep_id}");
    if !preview.is_empty() {
        println!("  Preview URL:   {preview}");
    }
    println!("\nFollow progress with: weoc logs {dep_id} --follow");
    Ok(())
}
