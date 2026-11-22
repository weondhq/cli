# weoc — Weond CLI

Official Rust command-line interface for [Weond](https://weond.com). Authenticate with OAuth 2.0 Device Authorization Flow, list workspace projects, trigger production deployments, and stream deployment logs directly from your terminal.

## Installation

### Build from source (Rust 1.85+)

```bash
git clone https://github.com/weondhq/cli.git
cd cli
cargo install --path .
```

Verify installation:

```bash
weoc --version
```

## Quickstart

```bash
# 1. Connect your Weond account via browser (Device Code Flow)
weoc connect

# 2. Verify active session
weoc whoami

# 3. List workspace projects
weoc projects list

# 4. Trigger a deployment
weoc deploy my-app

# 5. View or stream deployment logs
weoc logs <deployment-id>
weoc logs <deployment-id> --follow

# 6. Revoke session and remove local credentials
weoc disconnect
```

## Configuration

| Flag / Environment Variable | Default | Description |
| --- | --- | --- |
| `--api` / `WEOND_API_URL` | `https://api.weond.com` | Control plane API base URL (`https://` required for remote hosts; `http://localhost` allowed for local development) |
| `-v` / `--verbose` | `false` | Enable verbose diagnostic output |

## Security Design

- `#![forbid(unsafe_code)]` across the entire crate.
- **No off-origin token leaks**: HTTP redirects are strictly disabled so Bearer tokens are never forwarded to untrusted hosts.
- **TLS enforced**: Plaintext `http://` is rejected for all non-loopback hosts; embedded URL credentials are forbidden.
- **Hardened local credentials**: Tokens are stored in `<config_dir>/weoc/token.json` with `0700` directory and `0600` file permissions on Unix, and token memory is zeroized on drop.
- **Terminal injection protection**: All remote strings are stripped of ANSI escape sequences and C0/C1 control characters before printing.
- **Safe browser launch**: Verification URLs are validated against strict scheme, host, and character rules before launching the system browser.

## License

MIT — see [LICENSE](LICENSE).
