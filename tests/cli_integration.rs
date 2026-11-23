use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tempfile::tempdir;

#[derive(Debug, Clone)]
struct CapturedRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl CapturedRequest {
    fn header(&self, name: &str) -> Option<&str> {
        let target = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_ascii_lowercase() == target)
            .map(|(_, v)| v.as_str())
    }
}

struct MockServer {
    base_url: String,
    captured: Arc<Mutex<Vec<CapturedRequest>>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Drop for MockServer {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn spawn_mock_server<F>(expected_requests: usize, handler: F) -> MockServer
where
    F: Fn(usize, &CapturedRequest) -> (u16, String) + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_ref = Arc::clone(&captured);

    let handle = thread::spawn(move || {
        for idx in 0..expected_requests {
            let (mut stream, _) = listener.accept().unwrap();
            let req = read_http_request(&mut stream);
            let (status, resp_body) = handler(idx, &req);
            captured_ref.lock().unwrap().push(req);
            let reason = match status {
                200 => "OK",
                201 => "Created",
                401 => "Unauthorized",
                403 => "Forbidden",
                404 => "Not Found",
                _ => "Error",
            };
            let raw = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{resp_body}",
                resp_body.len()
            );
            stream.write_all(raw.as_bytes()).unwrap();
            stream.flush().unwrap();
        }
    });

    MockServer {
        base_url: format!("http://{addr}"),
        captured,
        handle: Some(handle),
    }
}

fn read_http_request(stream: &mut std::net::TcpStream) -> CapturedRequest {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end;
    loop {
        let n = stream.read(&mut chunk).unwrap();
        if n == 0 {
            panic!("unexpected EOF while reading HTTP headers");
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
    }

    let head_str = String::from_utf8_lossy(&buf[..header_end - 4]);
    let mut lines = head_str.split("\r\n");
    let request_line = lines.next().unwrap();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap().to_string();
    let path = parts.next().unwrap().to_string();

    let mut headers = Vec::new();
    let mut content_length = 0usize;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_string();
            let val = v.trim().to_string();
            if key.eq_ignore_ascii_case("content-length") {
                content_length = val.parse().unwrap_or(0);
            }
            headers.push((key, val));
        }
    }

    while buf.len() < header_end + content_length {
        let n = stream.read(&mut chunk).unwrap();
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }

    let body_bytes = &buf[header_end..(header_end + content_length).min(buf.len())];
    let body = String::from_utf8_lossy(body_bytes).to_string();

    CapturedRequest {
        method,
        path,
        headers,
        body,
    }
}

fn weoc_cmd(config_dir: &std::path::Path, cwd: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_weoc"));
    cmd.current_dir(cwd)
        .env_remove("WEOND_TOKEN")
        .env_remove("WEOND_API_URL")
        .env("XDG_CONFIG_HOME", config_dir)
        .env("HOME", config_dir);
    cmd
}

#[test]
fn cli_version_outputs_1_1_0() {
    let tmp = tempdir().unwrap();
    let out = weoc_cmd(tmp.path(), tmp.path())
        .arg("--version")
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "weoc 1.1.0");
}

#[test]
fn connect_token_saves_and_weond_token_overrides_without_persisting() {
    let tmp = tempdir().unwrap();
    let cfg_dir = tmp.path().join("cfg");
    let work_dir = tmp.path().join("work");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    std::fs::create_dir_all(&work_dir).unwrap();

    let server = spawn_mock_server(3, |idx, _req| {
        match idx {
        0 => (
            200,
            json!({
                "authenticated": true,
                "user": { "id": "u-1", "email": "dev@weond.com", "name": "Dev", "provider": "" }
            })
            .to_string(),
        ),
        1 => (
            200,
            json!({
                "authenticated": true,
                "user": { "id": "u-ci", "email": "ci@weond.com", "name": "CI", "provider": "" }
            })
            .to_string(),
        ),
        _ => (
            200,
            json!({
                "authenticated": true,
                "user": { "id": "u-1", "email": "dev@weond.com", "name": "Dev", "provider": "github" }
            })
            .to_string(),
        ),
    }
    });

    // 1. `weoc connect --token` validates via GET /req/auth/me and saves token.json.
    let conn = weoc_cmd(&cfg_dir, &work_dir)
        .args([
            "--api",
            &server.base_url,
            "connect",
            "--token",
            "pat_saved_1234567890",
        ])
        .output()
        .unwrap();
    assert!(
        conn.status.success(),
        "connect failed: {}",
        String::from_utf8_lossy(&conn.stderr)
    );
    let token_path = cfg_dir.join("weoc").join("token.json");
    assert!(token_path.exists());

    // 2. `WEOND_TOKEN` overrides token.json without overwriting it.
    let whoami_env = weoc_cmd(&cfg_dir, &work_dir)
        .env("WEOND_TOKEN", "pat_env_override_999")
        .args(["--api", &server.base_url, "whoami"])
        .output()
        .unwrap();
    assert!(whoami_env.status.success());
    let stdout_env = String::from_utf8_lossy(&whoami_env.stdout);
    assert!(stdout_env.contains("env: WEOND_TOKEN"));
    assert!(stdout_env.contains("API token"));

    // 3. Stored token remains intact and is used when WEOND_TOKEN is unset.
    let whoami_file = weoc_cmd(&cfg_dir, &work_dir)
        .args(["--api", &server.base_url, "whoami"])
        .output()
        .unwrap();
    assert!(whoami_file.status.success());
    let stdout_file = String::from_utf8_lossy(&whoami_file.stdout);
    assert!(stdout_file.contains("config: token.json"));
    assert!(stdout_file.contains("github"));

    let reqs = server.captured.lock().unwrap();
    assert_eq!(reqs[0].path, "/req/auth/me");
    assert_eq!(
        reqs[0].header("authorization"),
        Some("Bearer pat_saved_1234567890")
    );
    assert_eq!(
        reqs[1].header("authorization"),
        Some("Bearer pat_env_override_999")
    );
    assert_eq!(
        reqs[2].header("authorization"),
        Some("Bearer pat_saved_1234567890")
    );
}

#[test]
fn connect_token_stdin_rejects_401_without_writing_file() {
    let tmp = tempdir().unwrap();
    let cfg_dir = tmp.path().join("cfg");
    std::fs::create_dir_all(&cfg_dir).unwrap();

    let server = spawn_mock_server(1, |_idx, _req| {
        (
            401,
            json!({"error":{"message":"invalid token"}}).to_string(),
        )
    });

    let mut child = weoc_cmd(&cfg_dir, tmp.path())
        .args(["--api", &server.base_url, "connect", "--token-stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"pat_invalid\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(!out.status.success());
    assert!(!cfg_dir.join("weoc").join("token.json").exists());
}

#[test]
fn projects_link_deploy_rollback_env_domains_logs_and_disconnect_flow() {
    let tmp = tempdir().unwrap();
    let cfg_dir = tmp.path().join("cfg");
    let work_dir = tmp.path().join("project");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    std::fs::create_dir_all(&work_dir).unwrap();

    let projects_resp = json!({
        "data": {
            "projects": [{
                "id": "proj-1111",
                "name": "My App",
                "slug": "my-app",
                "workspaceId": "ws-2222",
                "repoFullName": "weondhq/my-app",
                "status": "ready",
                "previewUrl": "https://my-app.weond.app"
            }]
        }
    })
    .to_string();

    // Sequence of calls:
    // 0: projects list -> GET /api/projects
    // 1: link my-app -> GET /api/projects
    // 2: deploy (from .weond.json) -> POST /api/projects/my-app/deploy
    // 3..4: rollback dep-99 -> GET /api/projects, POST /req (RollbackDeployment)
    // 5..6: env list -e preview -> GET /api/projects, POST /req (WorkspaceCatalog)
    // 7..8: env set API_URL val -e preview -> GET /api/projects, POST /req (SetEnvVar)
    // 9..10: env rm API_URL -e preview -> GET /api/projects, POST /req (DeleteEnvVar)
    // 11..12: domains list -> GET /api/projects, POST /req (WorkspaceCatalog)
    // 13..14: domains add app.example.com -> GET /api/projects, POST /req (AddDomain)
    // 15: domains verify dom-1 -> POST /req (VerifyDomain)
    // 16: logs dep-99 -> GET /api/deployments/dep-99/logs?limit=25
    let server = spawn_mock_server(17, move |_idx, req| {
        if req.method == "GET" && req.path == "/api/projects" {
            return (200, projects_resp.clone());
        }
        if req.method == "POST"
            && (req.path == "/api/projects/my-app/deploy"
                || req.path == "/api/projects/my%2Dapp/deploy")
        {
            return (
                201,
                json!({
                    "data": {
                        "deploymentId": "dep-99",
                        "project": { "previewUrl": "https://my-app.weond.app" }
                    }
                })
                .to_string(),
            );
        }
        if req.method == "GET"
            && (req.path.starts_with("/api/deployments/dep-99/logs")
                || req.path.starts_with("/api/deployments/dep%2D99/logs"))
        {
            return (
                200,
                json!({
                    "data": {
                        "logs": [{
                            "id": "1",
                            "stream": "stdout",
                            "message": "build\u{001b}[31m ok",
                            "createdAt": "2026-11-23T15:00:00Z"
                        }]
                    }
                })
                .to_string(),
            );
        }
        if req.method == "POST" && req.path == "/req" {
            let body: Value = serde_json::from_str(&req.body).unwrap();
            let op = body
                .get("operationName")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match op {
                "RollbackDeployment" => {
                    return (
                        200,
                        json!({ "data": { "rollbackDeployment": { "activeDeploymentId": "dep-99" } } })
                            .to_string(),
                    );
                }
                "WorkspaceCatalog" => {
                    let q = body.get("query").and_then(|v| v.as_str()).unwrap_or("");
                    assert!(!q.contains("updatedAt"));
                    assert!(!q.contains("cnameTarget"));
                    return (
                        200,
                        json!({
                            "data": {
                                "catalog": {
                                    "projects": [{
                                        "id": "proj-1111",
                                        "env": [
                                            { "id": "e1", "key": "PREVIEW_KEY", "environments": ["preview"] },
                                            { "id": "e2", "key": "PROD_ONLY", "environments": ["production"] }
                                        ]
                                    }],
                                    "domains": [{
                                        "id": "dom-1",
                                        "projectId": "proj-1111",
                                        "hostname": "app.example.com",
                                        "status": "active",
                                        "sslStatus": "active"
                                    }]
                                }
                            }
                        })
                        .to_string(),
                    );
                }
                "SetEnvVar" => {
                    assert_eq!(body["variables"]["environments"], json!(["preview"]));
                    return (
                        200,
                        json!({ "data": { "setEnvVar": { "key": "API_URL" } } }).to_string(),
                    );
                }
                "DeleteEnvVar" => {
                    assert_eq!(body["variables"]["environments"], json!(["preview"]));
                    return (
                        200,
                        json!({ "data": { "deleteEnvVar": { "deleted": true } } }).to_string(),
                    );
                }
                "AddDomain" => {
                    return (
                        200,
                        json!({
                            "data": {
                                "addDomain": {
                                    "id": "dom-1",
                                    "hostname": "app.example.com",
                                    "status": "pending",
                                    "cname": { "name": "app", "value": "edge.weond.app" },
                                    "verification": { "type": "TXT", "name": "_weond", "value": "tok123" }
                                }
                            }
                        })
                        .to_string(),
                    );
                }
                "VerifyDomain" => {
                    return (
                        200,
                        json!({
                            "data": {
                                "verifyDomain": {
                                    "hostname": "app.example.com",
                                    "status": "active",
                                    "sslStatus": "active",
                                    "lastError": ""
                                }
                            }
                        })
                        .to_string(),
                    );
                }
                other => panic!("unexpected GraphQL op: {other}"),
            }
        }
        panic!("unexpected request: {} {}", req.method, req.path);
    });

    let run = |args: &[&str]| {
        weoc_cmd(&cfg_dir, &work_dir)
            .env("WEOND_TOKEN", "pat_ci_token")
            .arg("--api")
            .arg(&server.base_url)
            .args(args)
            .output()
            .unwrap()
    };

    let p = run(&["projects", "list"]);
    assert!(p.status.success());
    assert!(String::from_utf8_lossy(&p.stdout).contains("my-app"));

    let l = run(&["link", "my-app"]);
    assert!(l.status.success());
    assert!(work_dir.join(".weond.json").exists());

    let d = run(&["deploy"]);
    assert!(d.status.success());
    assert!(String::from_utf8_lossy(&d.stdout).contains("dep-99"));

    let rb = run(&["rollback", "dep-99"]);
    assert!(rb.status.success());
    assert!(String::from_utf8_lossy(&rb.stdout).contains("dep-99"));

    let el = run(&["env", "list", "-e", "preview"]);
    assert!(el.status.success());
    let el_out = String::from_utf8_lossy(&el.stdout);
    assert!(el_out.contains("PREVIEW_KEY"));
    assert!(!el_out.contains("PROD_ONLY"));

    let es = run(&[
        "env",
        "set",
        "API_URL",
        "https://api.example.com",
        "-e",
        "preview",
    ]);
    assert!(es.status.success());

    let er = run(&["env", "rm", "API_URL", "-e", "preview"]);
    assert!(er.status.success());

    let dl = run(&["domains", "list"]);
    assert!(dl.status.success());
    assert!(String::from_utf8_lossy(&dl.stdout).contains("app.example.com"));

    let da = run(&["domains", "add", "app.example.com"]);
    assert!(da.status.success());
    assert!(String::from_utf8_lossy(&da.stdout).contains("edge.weond.app"));

    let dv = run(&["domains", "verify", "dom-1"]);
    assert!(dv.status.success());

    let lg = run(&["logs", "dep-99", "-n", "25"]);
    assert!(lg.status.success());
    let lg_out = String::from_utf8_lossy(&lg.stdout);
    assert!(lg_out.contains("build ok"));
    assert!(!lg_out.contains("\x1b[31m"));

    // Disconnect notes WEOND_TOKEN without erroring when token.json is absent.
    let disc = run(&["disconnect"]);
    assert!(disc.status.success());
    assert!(String::from_utf8_lossy(&disc.stdout).contains("WEOND_TOKEN"));

    for req in server.captured.lock().unwrap().iter() {
        assert_eq!(req.header("authorization"), Some("Bearer pat_ci_token"));
    }
}

#[test]
fn logs_empty_and_follow_403_and_disconnect_revocation() {
    let tmp = tempdir().unwrap();
    let cfg_dir = tmp.path().join("cfg");
    std::fs::create_dir_all(cfg_dir.join("weoc")).unwrap();
    let token_path = cfg_dir.join("weoc").join("token.json");
    std::fs::write(&token_path, r#"{"access_token":"stored_tok_abc"}"#).unwrap();

    // 0: GET logs (empty) -> 200 {"data":{"logs":[]}}
    // 1: GET logs (initial for --follow) -> 200 {"data":{"logs":[]}}
    // 2: GET logs (poll in --follow) -> 403 {"error":{"message":"forbidden"}}
    // 3: POST /req/auth/logout -> 200 {"ok":true}
    let server = spawn_mock_server(4, |idx, req| match idx {
        0 | 1 => {
            assert_eq!(req.method, "GET");
            (200, json!({ "data": { "logs": [] } }).to_string())
        }
        2 => {
            assert_eq!(req.method, "GET");
            (
                403,
                json!({ "error": { "message": "forbidden" } }).to_string(),
            )
        }
        3 => {
            assert_eq!(req.method, "POST");
            assert_eq!(req.path, "/req/auth/logout");
            (200, json!({ "ok": true }).to_string())
        }
        _ => unreachable!(),
    });

    let empty = weoc_cmd(&cfg_dir, tmp.path())
        .args(["--api", &server.base_url, "logs", "dep-empty"])
        .output()
        .unwrap();
    assert!(empty.status.success());
    assert!(
        String::from_utf8_lossy(&empty.stdout)
            .contains("No logs found for deployment 'dep-empty'.")
    );

    let follow_err = weoc_cmd(&cfg_dir, tmp.path())
        .args(["--api", &server.base_url, "logs", "dep-empty", "--follow"])
        .output()
        .unwrap();
    assert!(!follow_err.status.success());
    assert!(String::from_utf8_lossy(&follow_err.stderr).contains("HTTP 403"));

    let disc = weoc_cmd(&cfg_dir, tmp.path())
        .args(["--api", &server.base_url, "disconnect"])
        .output()
        .unwrap();
    assert!(disc.status.success());
    assert!(String::from_utf8_lossy(&disc.stdout).contains("Account disconnected."));
    assert!(!token_path.exists());
}
