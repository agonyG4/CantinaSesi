//! CLI integration tests that exercise the real binary over HTTP.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_cantina-cli");

/// Parses `"GET /path?query HTTP/1.1"` into `(method, path_without_query)`.
fn parse_request_line(line: &str) -> (String, String) {
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let raw_path = parts.next().unwrap_or("/").to_string();
    let path = raw_path.split('?').next().unwrap_or(&raw_path).to_string();
    (method, path)
}

/// Starts a tiny HTTP server that accepts a single request and answers it
/// with the given router. The router receives the path without the query.
/// Returns the base URL and the serving thread.
fn start_server(
    route: impl Fn(&str) -> (u16, String) + Send + 'static,
) -> (String, std::thread::JoinHandle<()>) {
    start_server_raw(move |request_line| {
        let (_, path) = parse_request_line(request_line);
        route(&path)
    })
}

/// Starts a tiny HTTP server like [`start_server`] but passes the whole
/// request line (including the query string) to the router.
fn start_server_raw(
    route: impl Fn(&str) -> (u16, String) + Send + 'static,
) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();

    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0u8; 4096];
        let _ = stream.read(&mut buffer);

        let text = String::from_utf8_lossy(&buffer).to_string();
        let request_line = text.lines().next().unwrap_or("").to_string();
        let (status, body) = route(&request_line);

        let reason = if status == 200 { "OK" } else { "ERROR" };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });

    (format!("http://{address}"), handle)
}

/// Starts a server that advertises a 1000-byte body but sends only a few
/// bytes before closing, forcing the client body read to fail mid-transfer.
fn start_truncated_server(status: u16) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();

    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0u8; 4096];
        let _ = stream.read(&mut buffer);

        let reason = if status == 200 { "OK" } else { "ERROR" };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: 1000\r\nConnection: close\r\n\r\n{{\"error\":{{\"code\":\"INTERNAL\""
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });

    (format!("http://{address}"), handle)
}

/// Starts a tiny server like [`start_server`] but passes the whole raw
/// request (request line, headers and body) to the router.
fn start_server_raw_request(
    route: impl Fn(&str) -> (u16, String) + Send + 'static,
) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();

    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0u8; 8192];
        let _ = stream.read(&mut buffer);

        let text = String::from_utf8_lossy(&buffer).to_string();
        let (status, body) = route(&text);

        let reason = if status == 200 { "OK" } else { "ERROR" };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });

    (format!("http://{address}"), handle)
}

const HEALTH_BODY: &str = r#"{"status":"healthy","service":"cantina-api","version":"0.1.0"}"#;

const PRODUCTS_BODY: &str = r#"{"items":[
  {"id":"product_brigadeiro","name":"Brigadeiro","description":"Chocolate truffle, school classic","price":500,"category_id":"category_snacks","available":true},
  {"id":"product_coxinha","name":"Coxinha","description":"Fried shredded-chicken pastry","price":750,"category_id":"category_snacks","available":true},
  {"id":"product_refresco","name":"Refresco de limão","description":"Fresh lemonade","price":300,"category_id":"category_drinks","available":true}
]}"#;

const CATEGORIES_BODY: &str = r#"{"items":[
  {"id":"category_snacks","name":"Snacks"},
  {"id":"category_drinks","name":"Drinks"}
]}"#;

const NOT_FOUND_BODY: &str = r#"{"error":{"code":"NOT_FOUND","message":"the product with id 'x' was not found","request_id":"req-1"}}"#;

const LOGIN_BODY: &str = r#"{"access_token":"opaque-token-123","token_type":"Bearer","expires_at_epoch_seconds":1893456000,"user":{"id":"user_student_1","name":"Ana Silva","email":"ana@example.com","role":"student","status":"active"}}"#;

const ME_BODY: &str = r#"{"id":"user_student_1","name":"Ana Silva","email":"ana@example.com","role":"student","status":"active"}"#;

#[test]
fn version_command_prints_version() {
    let output = Command::new(BIN).arg("version").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.starts_with("cantina "),
        "unexpected stdout: {stdout}"
    );
    assert!(!stdout.trim().ends_with("cantina "));
}

#[test]
fn health_command_reaches_a_local_mock_api() {
    let (url, server) = start_server(move |_| (200, HEALTH_BODY.to_string()));

    let output = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("health")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("status: healthy"),
        "unexpected stdout: {stdout}"
    );
    assert!(stdout.contains("service: cantina-api"));

    server.join().unwrap();
}

#[test]
fn health_command_fails_when_the_api_cannot_be_reached() {
    let output = Command::new(BIN)
        .arg("--api-url")
        .arg("http://127.0.0.1:1")
        .arg("health")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "transport failure must exit non-zero"
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("unable to reach the API"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn invalid_api_url_is_reported() {
    let output = Command::new(BIN)
        .arg("--api-url")
        .arg("not a url")
        .arg("health")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("invalid API URL"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn products_list_prints_deterministic_table() {
    let (url, server) = start_server(move |path| {
        if path == "/api/v1/products" {
            (200, PRODUCTS_BODY.to_string())
        } else {
            (404, NOT_FOUND_BODY.to_string())
        }
    });

    let output = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("products")
        .arg("list")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("3 products"), "unexpected stdout: {stdout}");
    let header: Vec<&str> = stdout.lines().nth(1).unwrap().split_whitespace().collect();
    assert_eq!(
        header,
        vec!["ID", "NAME", "PRICE", "CATEGORY", "AVAILABILITY"],
        "a deterministic header must be rendered"
    );
    assert!(
        stdout.contains("product_brigadeiro"),
        "product ids must appear in the human output: {stdout}"
    );
    assert!(
        stdout.contains("product_refresco"),
        "product ids must appear in the human output: {stdout}"
    );
    assert!(stdout.contains("Brigadeiro"), "unexpected stdout: {stdout}");
    assert!(stdout.contains("R$ 5,00"), "unexpected stdout: {stdout}");
    assert!(stdout.contains("R$ 7,50"), "unexpected stdout: {stdout}");
    assert!(
        stdout.contains("category_snacks"),
        "category ids must appear in the human output: {stdout}"
    );

    server.join().unwrap();
}

#[test]
fn products_list_in_json_format_is_parseable() {
    let (url, server) = start_server(move |path| {
        if path == "/api/v1/products" {
            (200, PRODUCTS_BODY.to_string())
        } else {
            (404, NOT_FOUND_BODY.to_string())
        }
    });

    let output = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("--format")
        .arg("json")
        .arg("products")
        .arg("list")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(json["items"].as_array().unwrap().len(), 3);
    assert_eq!(json["items"][0]["price"], 500);

    server.join().unwrap();
}

#[test]
fn products_show_prints_single_product() {
    let (url, server) = start_server(move |path| {
        if path == "/api/v1/products/product_coxinha" {
            (
                200,
                r#"{"id":"product_coxinha","name":"Coxinha","description":"Fried shredded-chicken pastry","price":750,"category_id":"category_snacks","available":true}"#
                    .to_string(),
            )
        } else {
            (404, NOT_FOUND_BODY.to_string())
        }
    });

    let output = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("products")
        .arg("show")
        .arg("product_coxinha")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("name: Coxinha"),
        "unexpected stdout: {stdout}"
    );
    assert!(stdout.contains("R$ 7,50"), "unexpected stdout: {stdout}");

    server.join().unwrap();
}

#[test]
fn products_show_reports_api_error_responses() {
    let (url, server) = start_server(move |_| (404, NOT_FOUND_BODY.to_string()));

    let output = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("products")
        .arg("show")
        .arg("missing")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("HTTP 404"), "unexpected stderr: {stderr}");
    assert!(stderr.contains("not found"), "unexpected stderr: {stderr}");

    server.join().unwrap();
}

#[test]
fn categories_list_prints_human_rows() {
    let (url, server) = start_server(move |path| {
        if path == "/api/v1/categories" {
            (200, CATEGORIES_BODY.to_string())
        } else {
            (404, NOT_FOUND_BODY.to_string())
        }
    });

    let output = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("categories")
        .arg("list")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("2 categories"),
        "unexpected stdout: {stdout}"
    );
    assert!(stdout.contains("Snacks"), "unexpected stdout: {stdout}");
    assert!(stdout.contains("Drinks"), "unexpected stdout: {stdout}");

    server.join().unwrap();
}

#[test]
fn not_implemented_commands_exit_non_zero() {
    let output = Command::new(BIN)
        .arg("orders")
        .arg("list")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not implemented"));
}

#[test]
fn products_create_exits_non_zero_until_implemented() {
    let output = Command::new(BIN)
        .arg("products")
        .arg("create")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not implemented"));
}

#[test]
fn all_and_include_unavailable_send_the_same_query() {
    let all_url = run_query_probe_server();
    let alias_url = run_query_probe_server();

    let all = Command::new(BIN)
        .arg("--api-url")
        .arg(&all_url)
        .arg("products")
        .arg("list")
        .arg("--all")
        .output()
        .unwrap();
    assert!(
        all.status.success(),
        "--all must map to include_unavailable=true, stderr: {}",
        String::from_utf8_lossy(&all.stderr)
    );

    let alias = Command::new(BIN)
        .arg("--api-url")
        .arg(&alias_url)
        .arg("products")
        .arg("list")
        .arg("--include-unavailable")
        .output()
        .unwrap();
    assert!(
        alias.status.success(),
        "--include-unavailable must behave exactly like --all, stderr: {}",
        String::from_utf8_lossy(&alias.stderr)
    );
}

/// Starts a server that only accepts the canonical `include_unavailable=true`
/// query, proving whatever flag produced the request sent that exact query.
fn run_query_probe_server() -> String {
    let (url, server) = start_server_raw(move |request_line| {
        let raw_path = request_line.split_whitespace().nth(1).unwrap_or("/");
        let query = raw_path.split('?').nth(1).unwrap_or("");
        if query == "include_unavailable=true" {
            (200, PRODUCTS_BODY.to_string())
        } else {
            (
                400,
                format!(
                    r#"{{"error":{{"code":"INVALID_QUERY","message":"unexpected query: {query}"}}}}"#
                ),
            )
        }
    });
    drop(server);
    url
}

#[test]
fn products_list_help_documents_all_and_alias() {
    let output = Command::new(BIN)
        .arg("products")
        .arg("list")
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("--all"),
        "help must document the canonical --all flag: {stdout}"
    );
    assert!(
        stdout.contains("include-unavailable"),
        "help must document the --include-unavailable alias: {stdout}"
    );
}

#[test]
fn products_list_reports_internal_server_error() {
    let body = r#"{"error":{"code":"INTERNAL_ERROR","message":"internal server error","request_id":"req-9"}}"#;
    let (url, server) = start_server(move |_| (500, body.to_string()));

    let output = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("products")
        .arg("list")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("HTTP 500"), "unexpected stderr: {stderr}");
    assert!(
        stderr.contains("internal server error"),
        "structured error message must surface: {stderr}"
    );

    server.join().unwrap();
}

#[test]
fn products_list_with_malformed_success_body_reports_decode_error() {
    let (url, server) = start_server(move |_| (200, "this is not json".to_string()));

    let output = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("products")
        .arg("list")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("invalid response from the API"),
        "malformed success bodies must report a decode error: {stderr}"
    );

    server.join().unwrap();
}

#[test]
fn products_list_with_empty_success_body_reports_malformed_success() {
    let (url, server) = start_server(move |_| (200, String::new()));

    let output = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("products")
        .arg("list")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("without a response body"),
        "an empty 200 body must be reported explicitly: {stderr}"
    );

    server.join().unwrap();
}

#[test]
fn products_show_with_malformed_error_body_retains_status() {
    let (url, server) = start_server(move |_| (500, "not an error envelope".to_string()));

    let output = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("products")
        .arg("show")
        .arg("product_x")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("HTTP 500"),
        "the HTTP status must be retained: {stderr}"
    );
    assert!(
        !stderr.contains("UNKNOWN"),
        "malformed error bodies must not fall back to a fake UNKNOWN code: {stderr}"
    );

    server.join().unwrap();
}

#[test]
fn products_list_body_read_failure_is_reported() {
    let (url, server) = start_truncated_server(500);

    let output = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("products")
        .arg("list")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("failed to read the API response body"),
        "a mid-transfer body failure must be a typed error: {stderr}"
    );
    assert!(
        !stderr.contains("UNKNOWN"),
        "a body-read failure must never degrade to an UNKNOWN error: {stderr}"
    );

    server.join().unwrap();
}

#[test]
fn auth_login_with_password_stdin_posts_credentials_and_prints_session() {
    let (url, server) = start_server_raw_request(move |raw| {
        let lines: Vec<&str> = raw.lines().collect();
        let request_line = lines.first().unwrap_or(&"");
        assert!(
            request_line.starts_with("POST /api/v1/auth/login"),
            "expected login POST, got: {request_line}"
        );
        assert!(
            raw.contains("ana@example.com"),
            "email must be sent in the body: {raw}"
        );
        assert!(
            raw.contains("StudentDemo!2026"),
            "password must be sent in the body: {raw}"
        );
        (200, LOGIN_BODY.to_string())
    });

    let mut child = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("auth")
        .arg("login")
        .arg("--email")
        .arg("ana@example.com")
        .arg("--password-stdin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"StudentDemo!2026\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("opaque-token-123"),
        "the access token must be printed: {stdout}"
    );
    assert!(
        stdout.contains("expires_at_epoch_seconds: 1893456000"),
        "unexpected stdout: {stdout}"
    );
    assert!(
        stdout.contains("ana@example.com"),
        "unexpected stdout: {stdout}"
    );

    server.join().unwrap();
}

#[test]
fn auth_login_json_mode_prints_the_exact_api_response() {
    let (url, server) = start_server_raw_request(move |_| (200, LOGIN_BODY.to_string()));

    let mut child = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("--format")
        .arg("json")
        .arg("auth")
        .arg("login")
        .arg("--email")
        .arg("ana@example.com")
        .arg("--password-stdin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"StudentDemo!2026\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(json["access_token"], "opaque-token-123");
    assert_eq!(json["token_type"], "Bearer");
    assert_eq!(json["user"]["email"], "ana@example.com");

    server.join().unwrap();
}

#[test]
fn auth_login_with_empty_stdin_fails() {
    // The empty password aborts before any request, so no server is started.
    let mut child = Command::new(BIN)
        .arg("--api-url")
        .arg("http://127.0.0.1:1")
        .arg("auth")
        .arg("login")
        .arg("--email")
        .arg("ana@example.com")
        .arg("--password-stdin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(b"\n").unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("a password is required"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn auth_login_reports_api_error_responses() {
    let body = r#"{"error":{"code":"INVALID_CREDENTIALS","message":"invalid email or password","request_id":"req-7"}}"#;
    let (url, server) = start_server_raw_request(move |_| (401, body.to_string()));

    let mut child = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("auth")
        .arg("login")
        .arg("--email")
        .arg("ana@example.com")
        .arg("--password-stdin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"wrong-password\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("HTTP 401"), "unexpected stderr: {stderr}");
    assert!(
        stderr.contains("invalid email or password"),
        "structured error message must surface: {stderr}"
    );

    server.join().unwrap();
}

#[test]
fn auth_me_prints_user_table() {
    let (url, server) = start_server_raw_request(move |raw| {
        let lines: Vec<&str> = raw.lines().collect();
        let request_line = lines.first().unwrap_or(&"");
        assert!(
            request_line.starts_with("GET /api/v1/auth/me"),
            "expected me GET, got: {request_line}"
        );
        assert!(
            raw.contains("Bearer opaque-token-123"),
            "the bearer token must be attached: {raw}"
        );
        (200, ME_BODY.to_string())
    });

    let output = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("--token")
        .arg("opaque-token-123")
        .arg("auth")
        .arg("me")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let header: Vec<&str> = stdout.lines().next().unwrap().split_whitespace().collect();
    assert_eq!(
        header,
        vec!["ID", "NAME", "EMAIL", "ROLE", "STATUS"],
        "a deterministic header must be rendered"
    );
    assert!(
        stdout.contains("user_student_1"),
        "the user id must appear: {stdout}"
    );
    assert!(stdout.contains("Ana Silva"), "unexpected stdout: {stdout}");

    server.join().unwrap();
}

#[test]
fn auth_me_reads_token_from_environment() {
    let (url, server) = start_server_raw_request(move |raw| {
        assert!(
            raw.contains("Bearer env-token"),
            "the environment token must be attached: {raw}"
        );
        (200, ME_BODY.to_string())
    });

    let output = Command::new(BIN)
        .env("CANTINA_ACCESS_TOKEN", "env-token")
        .arg("--api-url")
        .arg(&url)
        .arg("auth")
        .arg("me")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().unwrap();
}

#[test]
fn auth_me_without_token_fails() {
    // The missing token aborts before any request, so no server is started.
    let output = Command::new(BIN)
        .arg("--api-url")
        .arg("http://127.0.0.1:1")
        .arg("auth")
        .arg("me")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("authentication required"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn auth_refresh_prints_a_new_session() {
    let (url, server) = start_server_raw_request(move |raw| {
        let lines: Vec<&str> = raw.lines().collect();
        let request_line = lines.first().unwrap_or(&"");
        assert!(
            request_line.starts_with("POST /api/v1/auth/refresh"),
            "expected refresh POST, got: {request_line}"
        );
        assert!(
            raw.contains("Bearer opaque-token-123"),
            "the bearer token must be attached: {raw}"
        );
        (200, LOGIN_BODY.to_string())
    });

    let output = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("--token")
        .arg("opaque-token-123")
        .arg("auth")
        .arg("refresh")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("opaque-token-123"),
        "the new access token must be printed: {stdout}"
    );

    server.join().unwrap();
}

#[test]
fn auth_logout_prints_logged_out() {
    let (url, server) = start_server_raw_request(move |raw| {
        let lines: Vec<&str> = raw.lines().collect();
        let request_line = lines.first().unwrap_or(&"");
        assert!(
            request_line.starts_with("POST /api/v1/auth/logout"),
            "expected logout POST, got: {request_line}"
        );
        assert!(
            raw.contains("Bearer opaque-token-123"),
            "the bearer token must be attached: {raw}"
        );
        (204, String::new())
    });

    let output = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("--token")
        .arg("opaque-token-123")
        .arg("auth")
        .arg("logout")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("logged out"), "unexpected stdout: {stdout}");

    server.join().unwrap();
}

#[test]
fn auth_failure_output_never_contains_the_token_or_password() {
    let body = r#"{"error":{"code":"INVALID_CREDENTIALS","message":"invalid email or password","request_id":"req-7"}}"#;
    let (url, server) = start_server_raw_request(move |_| (401, body.to_string()));

    let secret_password = "CorrectHorseBatteryStaple!99";
    let mut child = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("auth")
        .arg("login")
        .arg("--email")
        .arg("ana@example.com")
        .arg("--password-stdin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(format!("{secret_password}\n").as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        !stderr.contains(secret_password),
        "error output must never contain the plaintext password: {stderr}"
    );
    server.join().unwrap();

    let (url, server) = start_server_raw_request(move |_| (401, body.to_string()));
    let secret_token = "super-secret-opaque-token-xyz";
    let output = Command::new(BIN)
        .arg("--api-url")
        .arg(&url)
        .arg("--token")
        .arg(secret_token)
        .arg("auth")
        .arg("me")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        !stderr.contains(secret_token),
        "error output must never contain the plaintext token: {stderr}"
    );
    server.join().unwrap();
}

#[test]
fn auth_commands_do_not_create_a_token_persistence_file() {
    let dir = std::env::temp_dir().join(format!(
        "cantina-cli-nopersist-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let (url, server) = start_server_raw_request(move |_| (200, ME_BODY.to_string()));
    let output = Command::new(BIN)
        .current_dir(&dir)
        .arg("--api-url")
        .arg(&url)
        .arg("--token")
        .arg("opaque-token-123")
        .arg("auth")
        .arg("me")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("temp dir must exist")
        .collect();
    assert!(
        entries.is_empty(),
        "an authenticated command must not create any token persistence file"
    );

    let _ = std::fs::remove_dir_all(&dir);
    server.join().unwrap();
}
