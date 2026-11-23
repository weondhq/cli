# weoc — Weond CLI

Official Rust command-line interface for [Weond](https://weond.com). Authenticate with OAuth 2.0 Device Authorization Flow, list workspace projects, trigger production deployments, and stream deployment logs directly from your terminal.

## Installation

### macOS & Linux (Recommended)

```bash
curl -fsSL https://weond.com/cli | sh
```

### Windows (PowerShell)

```powershell
irm https://weond.com/cli.ps1 | iex
```

### Build from source (Rust 1.85+)

```bash
cargo install --git https://github.com/weondhq/cli.git
```

Or clone directly:

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

# 4. Link current directory to a project (.weond.json)
weoc link my-app

# 5. Trigger a deployment (uses linked project when slug is omitted)
weoc deploy

# 6. Manage encrypted environment variables & custom domains
weoc env list
weoc env set API_URL https://api.example.com
weoc domains list
weoc domains add example.com

# 7. View or live-stream deployment logs & roll back
weoc logs <deployment-id> --follow
weoc rollback <deployment-id>

# 8. Revoke session and remove local credentials
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
