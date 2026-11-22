use crate::commands::Context;
use crate::config::{load_token, remove_token};
use crate::http::build_client;
use anyhow::Result;
use reqwest::StatusCode;
use std::time::Duration;

pub fn run(ctx: &Context) -> Result<()> {
    if let Ok(token) = load_token() {
        let endpoint = format!("{}/req/auth/logout", ctx.api_base);
        if let Ok(client) = build_client(Duration::from_secs(5)) {
            match client
                .post(&endpoint)
                .bearer_auth(&token.access_token)
                .send()
            {
                Ok(resp) => {
                    let status = resp.status();
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

    if remove_token()? {
        println!("Account disconnected.");
    } else {
        println!("No account is currently connected.");
    }
    Ok(())
}
