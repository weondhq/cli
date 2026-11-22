use anyhow::{Context, Result, bail};
use reqwest::blocking::{Client, Response};
use reqwest::redirect::Policy;
use std::io::Read;
use std::time::Duration;
use url::Url;

pub const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024; // 4 MiB
pub const USER_AGENT: &str = concat!("weoc/", env!("CARGO_PKG_VERSION"));

/// Validates the control plane API base URL.
/// Enforces HTTPS for all remote hosts, allows HTTP only for loopback development,
/// and forbids embedded credentials.
pub fn validate_api_base_url(raw: &str) -> Result<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    let parsed = Url::parse(trimmed).with_context(|| format!("invalid API URL {raw:?}"))?;

    if !parsed.username().is_empty() || parsed.password().is_some() {
        bail!("API URL must not contain embedded credentials");
    }
    let Some(host) = parsed.host_str().map(|h| h.to_ascii_lowercase()) else {
        bail!("API URL must include a valid host");
    };

    match parsed.scheme() {
        "https" => Ok(trimmed.to_string()),
        "http" => {
            if matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1" | "[::1]") {
                Ok(trimmed.to_string())
            } else {
                bail!("refusing insecure HTTP API URL {raw:?}: use https:// for remote hosts");
            }
        }
        other => bail!("unsupported API URL scheme {other:?} (use https://)"),
    }
}

/// Builds a hardened HTTP client that refuses redirects so Bearer tokens never leak off-origin.
pub fn build_client(timeout: Duration) -> Result<Client> {
    Client::builder()
        .timeout(timeout)
        .redirect(Policy::none())
        .user_agent(USER_AGENT)
        .build()
        .context("failed to initialize HTTP client")
}

/// Reads a response body up to MAX_RESPONSE_BYTES to guard against unbounded memory growth.
pub fn read_bounded_body(resp: Response) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    resp.take((MAX_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut buf)
        .context("failed to read API response body")?;
    if buf.len() > MAX_RESPONSE_BYTES {
        bail!("API response exceeded maximum allowed size (4 MiB)");
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_api_urls() {
        assert_eq!(
            validate_api_base_url("https://api.weond.com/").unwrap(),
            "https://api.weond.com"
        );
        assert_eq!(
            validate_api_base_url("http://localhost:3000/").unwrap(),
            "http://localhost:3000"
        );
        assert_eq!(
            validate_api_base_url("http://127.0.0.1:3000").unwrap(),
            "http://127.0.0.1:3000"
        );
        assert!(validate_api_base_url("http://api.weond.com").is_err());
        assert!(validate_api_base_url("https://user:pass@api.weond.com").is_err());
        assert!(validate_api_base_url("ftp://api.weond.com").is_err());
    }
}
