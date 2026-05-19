use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    process::{Command, Output},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};

#[derive(Clone, Debug)]
struct RecordedRequest {
    request_line: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl RecordedRequest {
    fn method(&self) -> &str {
        self.request_line
            .split_whitespace()
            .next()
            .unwrap_or_default()
    }

    fn path(&self) -> &str {
        self.request_line
            .split_whitespace()
            .nth(1)
            .unwrap_or_default()
    }

    fn path_without_query(&self) -> &str {
        self.path()
            .split_once('?')
            .map_or(self.path(), |(path, _)| path)
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

struct MockResponse {
    status: u16,
    body: &'static str,
}

struct MockServer {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    handle: thread::JoinHandle<()>,
}

impl MockServer {
    fn spawn(
        expected_requests: usize,
        responder: impl Fn(&RecordedRequest) -> MockResponse + Send + Sync + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        listener
            .set_nonblocking(true)
            .expect("configure mock server");
        let addr = listener.local_addr().expect("read mock server address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let thread_requests = Arc::clone(&requests);
        let responder = Arc::new(responder);
        let handle = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while thread_requests.lock().unwrap().len() < expected_requests
                && Instant::now() < deadline
            {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .expect("configure mock request read timeout");
                        let request = read_request(&mut stream);
                        let response = responder(&request);
                        write_response(&mut stream, response);
                        thread_requests.lock().unwrap().push(request);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("mock server accept failed: {error}"),
                }
            }
        });

        Self {
            addr,
            requests,
            handle,
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn finish(self) -> Vec<RecordedRequest> {
        self.handle.join().expect("mock server thread failed");
        self.requests.lock().unwrap().clone()
    }
}

#[test]
fn gd_api_uses_bearer_token_from_gitcode_token() {
    let server = MockServer::spawn(1, |request| {
        if request.method() == "GET" && request.path_without_query() == "/api/v5/user" {
            MockResponse {
                status: 200,
                body: r#"{"login":"mock-user"}"#,
            }
        } else {
            MockResponse {
                status: 404,
                body: r#"{"message":"unexpected request"}"#,
            }
        }
    });
    let config_dir = tempfile::tempdir().expect("create temporary config dir");
    let api_base = format!("{}/api/v5", server.base_url());

    let mut command = gd_command();
    let output = command
        .env("GITCODE_TOKEN", "integration-token")
        .env("GD_CONFIG_PATH", config_dir.path().join("config.json"))
        .arg("--api-base")
        .arg(api_base)
        .args(["api", "/user", "--json"])
        .output()
        .expect("run gd api");

    let requests = server.finish();
    assert_command_success(&output, &requests);
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer integration-token")
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("integration-token"));
}

#[test]
fn gd_pipeline_list_reuses_gitcode_bearer_token() {
    let server = MockServer::spawn(2, |request| {
        match (request.method(), request.path_without_query()) {
            ("GET", "/api/v5/repos/owner/repo") => MockResponse {
                status: 200,
                body: r#"{"id":"42","http_url_to_repo":"https://gitcode.com/owner/repo.git"}"#,
            },
            ("POST", "/api/v2/projects/42/actions/workflows/list") => MockResponse {
                status: 200,
                body: r#"{"data":[],"total":0}"#,
            },
            _ => MockResponse {
                status: 404,
                body: r#"{"message":"unexpected request"}"#,
            },
        }
    });
    let config_dir = tempfile::tempdir().expect("create temporary config dir");
    let hostname = server.base_url();
    let api_base = format!("{hostname}/api/v5");

    let mut command = gd_command();
    let output = command
        .env("GITCODE_TOKEN", "integration-token")
        .env("GD_CONFIG_PATH", config_dir.path().join("config.json"))
        .arg("--hostname")
        .arg(&hostname)
        .arg("--api-base")
        .arg(api_base)
        .args(["pipeline", "list", "--repo", "owner/repo", "--json"])
        .output()
        .expect("run gd pipeline list");

    let requests = server.finish();
    assert_command_success(&output, &requests);
    assert_eq!(requests.len(), 2);
    assert!(
        requests
            .iter()
            .all(|request| { request.header("authorization") == Some("Bearer integration-token") })
    );
    assert!(
        requests
            .iter()
            .any(|request| request.path_without_query() == "/api/v5/repos/owner/repo")
    );
    let expected_referer = format!("{hostname}/");
    assert!(requests.iter().any(|request| {
        request.path_without_query() == "/api/v2/projects/42/actions/workflows/list"
            && request.body.contains(r#""page":1"#)
            && request.body.contains(r#""per_page":50"#)
            && request.header("referer") == Some(expected_referer.as_str())
    }));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("integration-token"));
}

#[test]
fn gd_pr_comment_and_reply_use_gitcode_bearer_token() {
    let server = MockServer::spawn(2, |request| {
        match (request.method(), request.path_without_query()) {
            ("POST", "/api/v5/repos/owner/repo/pulls/7/comments") => MockResponse {
                status: 200,
                body: r#"{"id":"discussion-1","body":"please fix"}"#,
            },
            ("POST", "/api/v5/repos/owner/repo/pulls/7/discussions/discussion-1/comments") => {
                MockResponse {
                    status: 200,
                    body: r#"{"id":"reply-1","body":"fixed"}"#,
                }
            }
            _ => MockResponse {
                status: 404,
                body: r#"{"message":"unexpected request"}"#,
            },
        }
    });
    let config_dir = tempfile::tempdir().expect("create temporary config dir");
    let api_base = format!("{}/api/v5", server.base_url());

    let mut comment_command = gd_command();
    let comment_output = comment_command
        .env("GITCODE_TOKEN", "integration-token")
        .env("GD_CONFIG_PATH", config_dir.path().join("config.json"))
        .arg("--api-base")
        .arg(&api_base)
        .args([
            "pr",
            "comment",
            "7",
            "--repo",
            "owner/repo",
            "--body",
            "please fix",
            "--path",
            "src/main.rs",
            "--position",
            "4",
            "--need-to-resolve",
            "--json",
        ])
        .output()
        .expect("run gd pr comment");

    let mut reply_command = gd_command();
    let reply_output = reply_command
        .env("GITCODE_TOKEN", "integration-token")
        .env("GD_CONFIG_PATH", config_dir.path().join("config.json"))
        .arg("--api-base")
        .arg(api_base)
        .args([
            "pr",
            "reply",
            "7",
            "discussion-1",
            "--repo",
            "owner/repo",
            "--body",
            "fixed",
            "--json",
        ])
        .output()
        .expect("run gd pr reply");

    let requests = server.finish();
    assert_command_success(&comment_output, &requests);
    assert_command_success(&reply_output, &requests);
    assert_eq!(requests.len(), 2);
    assert!(
        requests
            .iter()
            .all(|request| request.header("authorization") == Some("Bearer integration-token"))
    );
    assert!(requests.iter().any(|request| {
        request.path_without_query() == "/api/v5/repos/owner/repo/pulls/7/comments"
            && request.body.contains(r#""body":"please fix""#)
            && request.body.contains(r#""path":"src/main.rs""#)
            && request.body.contains(r#""position":4"#)
            && request.body.contains(r#""need_to_resolve":true"#)
    }));
    assert!(requests.iter().any(|request| {
        request.path_without_query()
            == "/api/v5/repos/owner/repo/pulls/7/discussions/discussion-1/comments"
            && request.body.contains(r#""body":"fixed""#)
    }));
}

#[test]
fn gd_pipeline_codecheck_creates_workflow_without_leaking_token() {
    let server = MockServer::spawn(2, |request| {
        match (request.method(), request.path_without_query()) {
            ("GET", "/api/v5/repos/owner/repo") => MockResponse {
                status: 200,
                body: r#"{"id":"42","default_branch":"main","http_url_to_repo":"https://gitcode.com/owner/repo.git"}"#,
            },
            ("POST", "/api/v5/repos/owner/repo/contents/.gitcode/workflows/codecheck.yml") => {
                MockResponse {
                    status: 200,
                    body: r#"{"commit":{"sha":"abc"}}"#,
                }
            }
            _ => MockResponse {
                status: 404,
                body: r#"{"message":"unexpected request"}"#,
            },
        }
    });
    let config_dir = tempfile::tempdir().expect("create temporary config dir");
    let api_base = format!("{}/api/v5", server.base_url());

    let mut command = gd_command();
    let output = command
        .env("GITCODE_TOKEN", "integration-token")
        .env("GD_CONFIG_PATH", config_dir.path().join("config.json"))
        .arg("--api-base")
        .arg(api_base)
        .args([
            "pipeline",
            "codecheck",
            "--repo",
            "owner/repo",
            "--language",
            "SHELL",
            "--access-token-secret",
            "CODECHECK_TOKEN",
            "--json",
        ])
        .output()
        .expect("run gd pipeline codecheck");

    let requests = server.finish();
    assert_command_success(&output, &requests);
    assert_eq!(requests.len(), 2);
    let workflow_request = requests
        .iter()
        .find(|request| {
            request.path_without_query()
                == "/api/v5/repos/owner/repo/contents/.gitcode/workflows/codecheck.yml"
        })
        .expect("workflow create request");
    let body: serde_json::Value =
        serde_json::from_str(&workflow_request.body).expect("parse workflow request body");
    let content = body["content"].as_str().expect("workflow content");
    let content = String::from_utf8(STANDARD.decode(content).expect("decode workflow content"))
        .expect("workflow content is utf-8");
    assert!(content.contains("uses: codecheck-action@0.0.3"));
    assert!(content.contains("access_token: '${{ secrets.CODECHECK_TOKEN }}'"));
    assert!(!content.contains("integration-token"));
}

#[test]
fn gd_api_uses_http_proxy_environment() {
    let target = MockServer::spawn(0, |_| MockResponse {
        status: 500,
        body: r#"{"message":"target should not receive proxied request"}"#,
    });
    let target_api_base = format!("{}/api/v5", target.base_url());
    let proxy = MockServer::spawn(1, |request| {
        if request.method() == "GET" && request.path().starts_with("http://") {
            MockResponse {
                status: 200,
                body: r#"{"login":"proxied-user"}"#,
            }
        } else {
            MockResponse {
                status: 404,
                body: r#"{"message":"unexpected proxy request"}"#,
            }
        }
    });
    let config_dir = tempfile::tempdir().expect("create temporary config dir");

    let mut command = gd_command();
    let output = command
        .env("GITCODE_TOKEN", "integration-token")
        .env("GD_CONFIG_PATH", config_dir.path().join("config.json"))
        .env("http_proxy", proxy.base_url())
        .arg("--api-base")
        .arg(target_api_base)
        .args(["api", "/user", "--json"])
        .output()
        .expect("run gd api through proxy");

    let requests = proxy.finish();
    let target_requests = target.finish();
    assert_command_success(&output, &requests);
    assert_eq!(requests.len(), 1);
    assert!(requests[0].path().contains("/api/v5/user"));
    assert!(target_requests.is_empty());
}

#[test]
fn gd_api_respects_no_proxy_environment() {
    let target = MockServer::spawn(1, |request| {
        if request.method() == "GET" && request.path_without_query() == "/api/v5/user" {
            MockResponse {
                status: 200,
                body: r#"{"login":"direct-user"}"#,
            }
        } else {
            MockResponse {
                status: 404,
                body: r#"{"message":"unexpected direct request"}"#,
            }
        }
    });
    let proxy = MockServer::spawn(0, |_| MockResponse {
        status: 500,
        body: r#"{"message":"proxy should not receive direct request"}"#,
    });
    let config_dir = tempfile::tempdir().expect("create temporary config dir");
    let api_base = format!("{}/api/v5", target.base_url());

    let mut command = gd_command();
    let output = command
        .env("GITCODE_TOKEN", "integration-token")
        .env("GD_CONFIG_PATH", config_dir.path().join("config.json"))
        .env("http_proxy", proxy.base_url())
        .env("NO_PROXY", "127.0.0.1")
        .arg("--api-base")
        .arg(api_base)
        .args(["api", "/user", "--json"])
        .output()
        .expect("run gd api with no_proxy");

    let requests = target.finish();
    let proxy_requests = proxy.finish();
    assert_command_success(&output, &requests);
    assert_eq!(requests.len(), 1);
    assert!(proxy_requests.is_empty());
}

fn gd_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gd"));
    for key in [
        "HTTP_PROXY",
        "http_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "ALL_PROXY",
        "all_proxy",
        "NO_PROXY",
        "no_proxy",
    ] {
        command.env_remove(key);
    }
    command
}

fn read_request(stream: &mut TcpStream) -> RecordedRequest {
    let mut buffer = Vec::new();
    let header_end = loop {
        let mut chunk = [0; 1024];
        let read = stream.read(&mut chunk).expect("read mock request");
        assert!(read != 0, "mock client closed connection before headers");
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(index) = find_header_end(&buffer) {
            break index;
        }
    };

    let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().expect("parse content length"))
        })
        .unwrap_or(0);
    let body_start = header_end + b"\r\n\r\n".len();
    while buffer.len() < body_start + content_length {
        let mut chunk = [0; 1024];
        let read = stream.read(&mut chunk).expect("read mock request body");
        assert!(read != 0, "mock client closed connection before body");
        buffer.extend_from_slice(&chunk[..read]);
    }

    let mut lines = headers.lines();
    let request_line = lines.next().unwrap_or_default().to_string();
    let headers = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect();
    let body =
        String::from_utf8_lossy(&buffer[body_start..body_start + content_length]).to_string();

    RecordedRequest {
        request_line,
        headers,
        body,
    }
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn write_response(stream: &mut TcpStream, response: MockResponse) {
    let reason = if response.status == 200 {
        "OK"
    } else {
        "Error"
    };
    let response = format!(
        "HTTP/1.1 {} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        response.status,
        response.body.len(),
        response.body
    );
    stream
        .write_all(response.as_bytes())
        .expect("write mock response");
}

fn assert_command_success(output: &Output, requests: &[RecordedRequest]) {
    assert!(
        output.status.success(),
        "command failed\nrequests:\n{requests:#?}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
