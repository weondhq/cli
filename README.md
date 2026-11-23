# weoc — Weond CLI

Official Rust command-line interface for [Weond](https://weond.com). Authenticate with OAuth 2.0 Device Authorization Flow or Personal Access Tokens (`WEOND_TOKEN` / `--token`), link local repositories, list workspace projects, trigger or roll back deployments, manage encrypted environment variables and custom domains, and inspect deployment logs directly from your terminal.

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
# weoc 1.1.0
```

## Quickstart

```bash
# 1. Connect your Weond account via browser (Device Code Flow)
weoc connect

# Or connect non-interactively with a Personal Access Token (validated before saving)
weoc connect --token <TOKEN>
printf '%s' "$WEOND_TOKEN" | weoc connect --token-stdin

# 2. Verify active session (works with token.json, WEOND_TOKEN, or --token)
weoc whoami

# 3. List workspace projects
weoc projects list

# 4. Link current directory to a project (.weond.json)
weoc link my-app

# 5. Trigger a deployment (uses linked project when [slug] is omitted)
weoc deploy
weoc deploy my-app

# 6. Manage encrypted environment variables (-p/--project, -e/--env)
weoc env list [-p my-app] [-e production]
weoc env set API_URL https://api.example.com [-p my-app] [-e production|preview|development]
weoc env rm API_URL [-p my-app] [-e production]

# 7. Manage custom domains (-p/--project)
weoc domains list [-p my-app]
weoc domains add example.com [-p my-app]
weoc domains verify <domain-id>

# 8. View or live-stream deployment logs & roll back
weoc logs <deployment-id> [-n 100] [--follow]
weoc rollback <deployment-id> [-p my-app]

# 9. Revoke stored CLI session and remove local credentials
weoc disconnect
```

## CI/CD Usage

Set `WEOND_TOKEN` in your CI environment or pass `--token <TOKEN>` on any command. When `WEOND_TOKEN` or `--token` is provided, it takes precedence over `<config_dir>/weoc/token.json` and is never written to disk unless you explicitly run `weoc connect --token`.

```bash
WEOND_TOKEN="$CI_WEOND_TOKEN" weoc deploy my-app
```

## Configuration

| Flag / Environment Variable | Default | Description |
| --- | --- | --- |
| `--api` / `WEOND_API_URL` | `https://api.weond.com` | Control plane API base URL (`https://` required for remote hosts; `http://localhost` / `127.0.0.1` allowed for local development) |
| `--token` / `WEOND_TOKEN` | — | Personal Access Token or API bearer token (overrides `token.json` without persisting to disk) |
| `-v` / `--verbose` | `false` | Enable verbose diagnostic output on `stderr` |

## Security Design

- `#![forbid(unsafe_code)]` across the entire crate.
- **No off-origin token leaks**: HTTP redirects are strictly disabled so Bearer tokens are never forwarded to untrusted hosts.
- **TLS enforced**: Plaintext `http://` is rejected for all non-loopback hosts; embedded URL credentials are forbidden.
- **Hardened local credentials**: Tokens are stored in `<config_dir>/weoc/token.json` with `0700` directory and `0600` file permissions on Unix, and token memory is zeroized on drop.
- **Terminal injection protection**: All remote strings are stripped of ANSI escape sequences and C0/C1 control characters before printing.
- **Safe browser launch**: Verification URLs are validated against strict scheme, host, and character rules before launching the system browser.

## License

MIT — see [LICENSE](LICENSE).
