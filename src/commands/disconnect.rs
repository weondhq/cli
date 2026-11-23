use crate::commands::Context;
use crate::config::{load_token, remove_token};
use crate::http::{build_client, verbose_log};
use anyhow::Result;
use reqwest::StatusCode;
use std::time::Duration;

pub fn run(ctx: &Context) -> Result<()> {
    if let Ok(token) = load_token() {
        let endpoint = format!("{}/req/auth/logout", ctx.api_base);
        verbose_log(ctx.verbose, &format!("POST {endpoint}"));
        if let Ok(client) = build_client(Duration::from_secs(5)) {
            match client
                .post(&endpoint)
                .bearer_auth(&token.access_token)
                .send()
            {
                Ok(resp) => {
                    let status = resp.status();
                    verbose_log(
                        ctx.verbose,
                        &format!("POST {endpoint} -> HTTP {}", status.as_u16()),
                    );
                    if status.is_client_error() && status != StatusCode::UNAUTHORIZED
                        || status.is_server_error()
                    {
                        eprintln!(
                            "Warning: remote session revocation returned HTTP {}; removing local credentials.",
                            status.as_u16()
                        );
                    }
                }
                Err(err) => {
                    eprintln!(
                        "Warning: could not revoke session remotely ({err}); removing local credentials."
                    );
                }
            }
        }
    }

    let removed = remove_token()?;
    if removed {
        println!("Account disconnected.");
    } else {
        println!("No stored credentials found in token.json.");
    }

    if std::env::var_os("WEOND_TOKEN").is_some_and(|v| !v.is_empty()) {
        println!("Note: WEOND_TOKEN is set in your environment and was not unset.");
    }
    Ok(())
}
