use crate::commands::Context;
use crate::http::{build_client, read_bounded_body, verbose_log};
use crate::sanitize::sanitize_terminal_text;
use anyhow::{Context as _, Result, bail};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::StatusCode;
use serde::Deserialize;
use std::thread;
use std::time::Duration;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeploymentLog {
    #[serde(default)]
    id: String,
    #[serde(default)]
    stream: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct LogsEnvelope {
    data: LogsData,
}

#[derive(Debug, Deserialize)]
struct LogsData {
    #[serde(default)]
    logs: Vec<DeploymentLog>,
}

pub fn run(ctx: &Context, deployment_id: &str, follow: bool, limit: u16) -> Result<()> {
    let trimmed_id = deployment_id.trim();
    if trimmed_id.is_empty() {
        bail!("deployment ID cannot be empty");
    }

    let auth = ctx.resolve_auth()?;
    let encoded_id = utf8_percent_encode(trimmed_id, NON_ALPHANUMERIC).to_string();
    let endpoint = format!(
        "{}/api/deployments/{encoded_id}/logs?limit={limit}",
        ctx.api_base
    );
    verbose_log(ctx.verbose, &format!("GET {endpoint}"));

    let client = build_client(Duration::from_secs(15))?;
    let resp = client
        .get(&endpoint)
        .bearer_auth(auth.access_token.as_str())
        .send()
        .context("failed to fetch deployment logs")?;

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
            "failed to load logs (HTTP {}): {}",
            status.as_u16(),
            sanitize_terminal_text(&String::from_utf8_lossy(&body))
        );
    }

    let result: LogsEnvelope =
        serde_json::from_slice(&body).context("failed to decode logs response")?;

    let mut last_id = String::new();
    for entry in &result.data.logs {
        last_id = entry.id.clone();
        print_log_line(entry);
    }

    if result.data.logs.is_empty() && !follow {
        println!(
            "No logs found for deployment '{}'.",
            sanitize_terminal_text(trimmed_id)
        );
        return Ok(());
    }

    if follow {
        println!("\nStreaming live logs... (Press Ctrl+C to exit)");
        let poll_client = build_client(Duration::from_secs(10))?;
        loop {
            thread::sleep(Duration::from_secs(2));
            let encoded_after = utf8_percent_encode(&last_id, NON_ALPHANUMERIC).to_string();
            let poll_url = format!(
                "{}/api/deployments/{encoded_id}/logs?limit=50&after={encoded_after}",
                ctx.api_base
            );

            let Ok(poll_resp) = poll_client
                .get(&poll_url)
                .bearer_auth(auth.access_token.as_str())
                .send()
            else {
                continue;
            };
            let poll_status = poll_resp.status();
            if poll_status == StatusCode::UNAUTHORIZED {
                bail!("session token expired while streaming logs");
            }
            if poll_status == StatusCode::FORBIDDEN {
                bail!("permission denied while streaming logs (HTTP 403)");
            }
            if poll_status == StatusCode::NOT_FOUND {
                bail!(
                    "deployment '{}' not found while streaming logs (HTTP 404)",
                    sanitize_terminal_text(trimmed_id)
                );
            }
            if poll_status != StatusCode::OK {
                continue;
            }
            let Ok(poll_body) = read_bounded_body(poll_resp) else {
                continue;
            };
            let Ok(parsed) = serde_json::from_slice::<LogsEnvelope>(&poll_body) else {
                continue;
            };
            for entry in &parsed.data.logs {
                if !last_id.is_empty() && entry.id == last_id {
                    continue;
                }
                last_id = entry.id.clone();
                print_log_line(entry);
            }
        }
    }

    Ok(())
}

fn print_log_line(entry: &DeploymentLog) {
    let level = if entry.stream == "stderr" {
        "ERR "
    } else {
        "INFO"
    };
    let ts_clean = sanitize_terminal_text(&entry.created_at);
    let ts: String = ts_clean.chars().take(19).collect();
    let msg = sanitize_terminal_text(&entry.message);
    println!("[{level}] [{ts}] {msg}");
}
