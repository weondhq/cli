use crate::commands::Context;
use crate::http::{build_client, read_bounded_body, verbose_log};
use crate::sanitize::sanitize_terminal_text;
use anyhow::{Context as _, Result, bail};
use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Project {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    repo_full_name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    preview_url: String,
}

#[derive(Debug, Deserialize)]
struct ProjectsEnvelope {
    data: ProjectsData,
}

#[derive(Debug, Deserialize)]
struct ProjectsData {
    #[serde(default)]
    projects: Vec<Project>,
}

pub fn run_list(ctx: &Context) -> Result<()> {
    let auth = ctx.resolve_auth()?;
    let endpoint = format!("{}/api/projects", ctx.api_base);
    verbose_log(ctx.verbose, &format!("GET {endpoint}"));
    let client = build_client(Duration::from_secs(15))?;
    let resp = client
        .get(&endpoint)
        .bearer_auth(auth.access_token.as_str())
        .send()
        .context("failed to list projects")?;

    let status = resp.status();
    verbose_log(
        ctx.verbose,
        &format!("GET {endpoint} -> HTTP {}", status.as_u16()),
    );
    let body = read_bounded_body(resp)?;

    if status == StatusCode::UNAUTHORIZED {
        bail!("session token is invalid or expired; run 'weoc connect' or check WEOND_TOKEN");
    }
    if status != StatusCode::OK {
        bail!(
            "failed to list projects (HTTP {}): {}",
            status.as_u16(),
            sanitize_terminal_text(&String::from_utf8_lossy(&body))
        );
    }

    let envelope: ProjectsEnvelope =
        serde_json::from_slice(&body).context("failed to decode projects response")?;

    if envelope.data.projects.is_empty() {
        println!("No projects yet. Create one from the web dashboard.");
        return Ok(());
    }

    let rows: Vec<[String; 6]> = envelope
        .data
        .projects
        .iter()
        .map(|p| {
            let clean_id = sanitize_terminal_text(&p.id);
            let short_id = if clean_id.chars().count() > 8 {
                let prefix: String = clean_id.chars().take(8).collect();
                format!("{prefix}...")
            } else {
                clean_id
            };
            [
                short_id,
                sanitize_terminal_text(&p.name),
                sanitize_terminal_text(&p.slug),
                sanitize_terminal_text(&p.repo_full_name),
                sanitize_terminal_text(&p.status),
                sanitize_terminal_text(&p.preview_url),
            ]
        })
        .collect();

    let headers = ["ID", "NAME", "SLUG", "REPO", "STATUS", "PREVIEW"];
    let mut widths = headers.map(str::len);
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }

    println!(
        "{:<w0$}  {:<w1$}  {:<w2$}  {:<w3$}  {:<w4$}  {}",
        headers[0],
        headers[1],
        headers[2],
        headers[3],
        headers[4],
        headers[5],
        w0 = widths[0],
        w1 = widths[1],
        w2 = widths[2],
        w3 = widths[3],
        w4 = widths[4],
    );
    for row in &rows {
        println!(
            "{:<w0$}  {:<w1$}  {:<w2$}  {:<w3$}  {:<w4$}  {}",
            row[0],
            row[1],
            row[2],
            row[3],
            row[4],
            row[5],
            w0 = widths[0],
            w1 = widths[1],
            w2 = widths[2],
            w3 = widths[3],
            w4 = widths[4],
        );
    }

    Ok(())
}
