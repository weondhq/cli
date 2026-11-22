use url::Url;

/// Strips ANSI escape sequences and C0/C1 control characters from remote text
/// before printing it to the terminal.
pub fn sanitize_terminal_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            match chars.peek().copied() {
                // CSI sequence: ESC [ ... final byte 0x40..=0x7e
                Some('[') => {
                    chars.next();
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                // OSC sequence: ESC ] ... BEL or ST (ESC \)
                Some(']') => {
                    chars.next();
                    let mut prev_esc = false;
                    for c in chars.by_ref() {
                        if c == '\x07' || (prev_esc && c == '\\') {
                            break;
                        }
                        prev_esc = c == '\x1b';
                    }
                }
                // Two-byte escape sequence
                Some(_) => {
                    chars.next();
                }
                None => {}
            }
            continue;
        }

        if ch == '\t' {
            out.push(' ');
            continue;
        }
        let code = ch as u32;
        if code < 0x20 || code == 0x7f || (0x80..=0x9f).contains(&code) {
            continue;
        }
        out.push(ch);
    }

    out
}

/// Masks an access token for safe terminal display.
pub fn mask_access_token(token: &str) -> String {
    let chars: Vec<char> = token.chars().collect();
    if chars.len() <= 16 {
        return "****".to_string();
    }
    let head: String = chars[..8].iter().collect();
    let tail: String = chars[chars.len() - 8..].iter().collect();
    format!("{head}...{tail}")
}

/// Validates that a verification URL is safe to open automatically in the user's browser.
/// Only allows HTTPS URLs (or loopback HTTP for local development) without shell metacharacters,
/// embedded credentials, or untrusted hosts outside Weond / the configured API host.
pub fn is_safe_browser_url(raw: &str, api_base: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed
            .chars()
            .any(|c| (c as u32) < 0x20 || (c as u32) == 0x7f || " \t\r\n\"'<>\\`;&|".contains(c))
    {
        return false;
    }

    let Ok(parsed) = Url::parse(trimmed) else {
        return false;
    };
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return false;
    }
    let Some(host) = parsed.host_str().map(|h| h.to_ascii_lowercase()) else {
        return false;
    };

    let is_loopback = matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1" | "[::1]");
    let api_is_loopback = Url::parse(api_base)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
        .is_some_and(|h| matches!(h.as_str(), "localhost" | "127.0.0.1" | "::1" | "[::1]"));

    match parsed.scheme() {
        "https" => {}
        "http" if is_loopback && api_is_loopback => return true,
        _ => return false,
    }

    if host == "weond.com" || host.ends_with(".weond.com") {
        return true;
    }

    if let Ok(api_url) = Url::parse(api_base)
        && let Some(api_host) = api_url.host_str().map(|h| h.to_ascii_lowercase())
        && host == api_host
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_ansi_and_control_chars() {
        let input = "\x1b[31mred\x1b[0m\x1b]0;evil-title\x07\r\nok\tline\x7f";
        assert_eq!(sanitize_terminal_text(input), "redok line");
    }

    #[test]
    fn masks_tokens() {
        assert_eq!(mask_access_token("short"), "****");
        assert_eq!(
            mask_access_token("12345678abcdefgh98765432"),
            "12345678...98765432"
        );
    }

    #[test]
    fn validates_browser_urls() {
        let api = "https://api.weond.com";
        assert!(is_safe_browser_url(
            "https://weond.com/cli/connect?user_code=ABCD",
            api
        ));
        assert!(is_safe_browser_url(
            "http://localhost:5173/cli/connect",
            "http://localhost:3000"
        ));
        assert!(!is_safe_browser_url(
            "http://localhost:5173/cli/connect",
            api
        ));
        assert!(!is_safe_browser_url("javascript:alert(1)", api));
        assert!(!is_safe_browser_url("https://evil.example.com/phish", api));
        assert!(!is_safe_browser_url(
            "https://user:pass@weond.com/cli/connect",
            api
        ));
        assert!(!is_safe_browser_url(
            "https://weond.com/cli/connect;whoami",
            api
        ));
    }
}
