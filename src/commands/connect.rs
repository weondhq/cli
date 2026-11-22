use crate::commands::Context;
use crate::config::{StoredToken, save_token};
use crate::http::{build_client, read_bounded_body};
use crate::sanitize::{is_safe_browser_url, mask_access_token, sanitize_terminal_text};
use anyhow::{Context as _, Result, bail};
use reqwest::StatusCode;
use serde::Deserialize;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    verification_url_complete: String,
    expires_in: u64,
    #[serde(default = "default_interval")]
    interval: u64,
}

fn default_interval() -> u64 {
    5
}

#[derive(Debug, Deserialize)]
struct TokenErrorBody {
    #[serde(default)]
    error: String,
}

pub fn run(ctx: &Context) -> Result<()> {
    println!("Starting Weond CLI connection...");
    let device = request_device_code(ctx)?;
    let verification_url = if device.verification_url_complete.trim().is_empty() {
        format!("{}?user_code={}", device.verification_uri, device.user_code)
    } else {
        device.verification_url_complete.clone()
    };

    print_connection_info(&verification_url, &device.user_code, device.expires_in);
    open_browser(&verification_url, &ctx.api_base, ctx.verbose);

    println!("\nWaiting for authorization... (Press Ctrl+C to cancel)");
    let token = poll_for_token(ctx, &device)?;
    save_token(&token)?;

    println!("\nConnected successfully.");
    println!("  Access token: {}", mask_access_token(&token.access_token));
    println!(
        "  Token type:   {}",
        sanitize_terminal_text(&token.token_type)
    );
    println!("  Expires in:   {}s", token.expires_in);
    Ok(())
}

fn request_device_code(ctx: &Context) -> Result<DeviceCodeResponse> {
    let endpoint = format!("{}/api/cli/device/code", ctx.api_base);
    let client = build_client(Duration::from_secs(10))?;
    let resp = client
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .send()
        .context("failed to reach Weond API")?;

    let status = resp.status();
    let body = read_bounded_body(resp)?;
    if status != StatusCode::OK {
        bail!(
            "device code request failed (HTTP {}): {}",
            status.as_u16(),
            sanitize_terminal_text(&String::from_utf8_lossy(&body))
        );
    }

    serde_json::from_slice(&body).context("failed to decode device code response")
}

fn print_connection_info(verification_url: &str, user_code: &str, expires_in: u64) {
    let safe_url = sanitize_terminal_text(verification_url);
    let safe_code = sanitize_terminal_text(user_code);
    let bar = "=".repeat(60);
    let sep = "-".repeat(60);

    println!("\n{bar}");
    println!("  WEOND CLI ACCOUNT AUTHORIZATION");
    println!("{bar}");
    println!("\n  Verification URL:  {safe_url}");
    println!("  Verification Code: {safe_code}");
    println!("\n{sep}");
    println!("  Steps:");
    println!("  1. Open the verification URL in your browser");
    println!("  2. Confirm the code matches: {safe_code}");
    println!("  3. Click \"Authorize CLI\" to complete sign-in");
    println!("{bar}");
    println!("\n  Code expires in {} minutes", (expires_in / 60).max(1));
}

fn open_browser(raw_url: &str, api_base: &str, verbose: bool) {
    if !is_safe_browser_url(raw_url, api_base) {
        if verbose {
            eprintln!("  Refusing to auto-open untrusted or non-HTTPS verification URL.");
        }
        return;
    }

    #[cfg(target_os = "linux")]
    let res = Command::new("xdg-open")
        .arg(raw_url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    #[cfg(target_os = "macos")]
    let res = Command::new("open")
        .arg(raw_url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    #[cfg(target_os = "windows")]
    let res = Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", raw_url])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let res: std::io::Result<std::process::Child> = Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "unsupported platform",
    ));

    if let Err(err) = res
        && verbose
    {
        eprintln!("  Could not open browser automatically: {err}");
    }
}

fn poll_for_token(ctx: &Context, device: &DeviceCodeResponse) -> Result<StoredToken> {
    let endpoint = format!("{}/api/cli/device/token", ctx.api_base);
    let client = build_client(Duration::from_secs(10))?;
    let mut interval = Duration::from_secs(device.interval.max(5));
    let deadline = Instant::now() + Duration::from_secs(device.expires_in.max(60));

    loop {
        thread::sleep(interval);
        if Instant::now() >= deadline {
            bail!("verification code expired; run 'weoc connect' again");
        }

        let payload = serde_json::json!({
            "device_code": device.device_code,
            "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
        });

        let resp = match client.post(&endpoint).json(&payload).send() {
            Ok(r) => r,
            Err(err) => {
                if ctx.verbose {
                    eprintln!("  Network retry: {err}");
                }
                continue;
            }
        };

        let status = resp.status();
        let body = read_bounded_body(resp)?;

        if status == StatusCode::BAD_REQUEST {
            let parsed: TokenErrorBody = serde_json::from_slice(&body).unwrap_or(TokenErrorBody {
                error: String::new(),
            });
            match parsed.error.as_str() {
                "authorization_pending" => continue,
                "slow_down" => {
                    interval += Duration::from_secs(5);
                    continue;
                }
                "expired_token" => {
                    bail!("verification code expired; run 'weoc connect' again");
                }
                other => {
                    bail!("authorization failed: {}", sanitize_terminal_text(other));
                }
            }
        }

        if status != StatusCode::OK {
            bail!("API error while polling token (HTTP {})", status.as_u16());
        }

        let token: StoredToken =
            serde_json::from_slice(&body).context("failed to decode token response")?;
        if token.access_token.trim().is_empty() {
            bail!("API returned an empty access token");
        }
        return Ok(token);
    }
}
