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

/// Emits a verbose diagnostic line to stderr without touching stdout.
pub fn verbose_log(verbose: bool, message: &str) {
    if verbose {
        eprintln!("[verbose] {message}");
    }
}

/// Executes an authenticated GraphQL request against POST /req using a Bearer CLI token.
pub fn graphql_request(
    api_base: &str,
    access_token: &str,
    verbose: bool,
    operation_name: &str,
    query: &str,
    variables: serde_json::Value,
) -> Result<serde_json::Value> {
    let client = build_client(Duration::from_secs(30))?;
    let endpoint = format!("{api_base}/req");
    verbose_log(verbose, &format!("POST {endpoint} ({operation_name})"));
    let payload = serde_json::json!({
        "operationName": operation_name,
        "query": query,
        "variables": variables,
    });
    let resp = client
        .post(&endpoint)
        .bearer_auth(access_token)
        .json(&payload)
        .send()
        .context("failed to contact control plane API")?;

    let status = resp.status();
    verbose_log(
        verbose,
        &format!("POST {endpoint} -> HTTP {}", status.as_u16()),
    );
    let body = read_bounded_body(resp)?;
    if status == reqwest::StatusCode::UNAUTHORIZED {
        bail!("session token is invalid or expired; run 'weoc connect' or check WEOND_TOKEN");
    }

    let parsed: serde_json::Value = serde_json::from_slice(&body)
        .with_context(|| format!("invalid JSON response (HTTP {})", status.as_u16()))?;

    if let Some(err_msg) = parsed
        .get("errors")
        .and_then(|e| e.as_array())
        .and_then(|arr| arr.first())
        .and_then(|first| first.get("message"))
        .and_then(|m| m.as_str())
    {
        bail!("{}", crate::sanitize::sanitize_terminal_text(err_msg));
    }

    if let Some(err_msg) = parsed
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
    {
        bail!("{}", crate::sanitize::sanitize_terminal_text(err_msg));
    }

    if !status.is_success() {
        bail!("request failed (HTTP {})", status.as_u16());
    }

    parsed
        .get("data")
        .cloned()
        .context("API response missing data field")
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

    #[test]
    fn graphql_request_extracts_rest_error_and_sanitizes_graphql_errors() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let handle = std::thread::spawn(move || {
            for idx in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let (status_line, body) = if idx == 0 {
                    (
                        "HTTP/1.1 403 Forbidden",
                        r#"{"error":{"code":"forbidden","message":"Role\u001b[31m denied"}}"#,
                    )
                } else {
                    (
                        "HTTP/1.1 200 OK",
                        r#"{"errors":[{"message":"bad\u001b[2J input"}]}"#,
                    )
                };
                let resp = format!(
                    "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(resp.as_bytes()).unwrap();
            }
        });

        let api_base = format!("http://{addr}");
        let err1 = graphql_request(
            &api_base,
            "tok",
            false,
            "Op1",
            "query { x }",
            serde_json::json!({}),
        )
        .unwrap_err();
        assert_eq!(err1.to_string(), "Role denied");

        let err2 = graphql_request(
            &api_base,
            "tok",
            false,
            "Op2",
            "query { y }",
            serde_json::json!({}),
        )
        .unwrap_err();
        assert_eq!(err2.to_string(), "bad input");

        handle.join().unwrap();
    }
}
