use crate::commands::Context;
use crate::config::load_token;
use crate::http::{build_client, read_bounded_body};
use crate::sanitize::sanitize_terminal_text;
use anyhow::{Context as _, Result, bail};
use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserInfo {
    #[serde(default)]
    id: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    avatar_url: String,
    #[serde(default)]
    provider: String,
}

#[derive(Debug, Default, Deserialize)]
struct MeData {
    #[serde(default)]
    authenticated: bool,
    #[serde(default)]
    user: Option<UserInfo>,
}

#[derive(Debug, Deserialize)]
struct MeResponse {
    #[serde(default)]
    authenticated: bool,
    #[serde(default)]
    user: Option<UserInfo>,
    #[serde(default)]
    data: Option<MeData>,
}

pub fn run(ctx: &Context) -> Result<()> {
    let token = match load_token() {
        Ok(tok) => tok,
        Err(_) => {
            println!("Not connected.");
            println!("  Run 'weoc connect' to authorize your CLI.");
            return Ok(());
        }
    };

    let endpoint = format!("{}/req/auth/me", ctx.api_base);
    let client = build_client(Duration::from_secs(10))?;
    let resp = client
        .get(&endpoint)
        .bearer_auth(&token.access_token)
        .send()
        .context("failed to contact Weond API")?;

    let status = resp.status();
    let body = read_bounded_body(resp)?;

    if status == StatusCode::UNAUTHORIZED {
        println!("Session token is invalid or expired.");
        println!("  Run 'weoc connect' to re-authorize.");
        return Ok(());
    }
    if status != StatusCode::OK {
        bail!(
            "unable to retrieve account info (HTTP {}): {}",
            status.as_u16(),
            sanitize_terminal_text(&String::from_utf8_lossy(&body))
        );
    }

    let parsed: MeResponse =
        serde_json::from_slice(&body).context("failed to decode account response")?;

    let nested = parsed.data.unwrap_or_default();
    let authenticated = parsed.authenticated || nested.authenticated;
    let user = parsed.user.or(nested.user).unwrap_or_default();

    if !authenticated || user.id.is_empty() {
        println!("Not signed in.");
        return Ok(());
    }

    println!("Connected account:");
    println!("  Name:     {}", sanitize_terminal_text(&user.name));
    println!("  Email:    {}", sanitize_terminal_text(&user.email));
    println!("  Provider: {}", sanitize_terminal_text(&user.provider));
    println!("  ID:       {}", sanitize_terminal_text(&user.id));
    if !user.avatar_url.is_empty() {
        println!("  Avatar:   {}", sanitize_terminal_text(&user.avatar_url));
    }
    Ok(())
}
