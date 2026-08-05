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
/// with the given router. Returns the base URL and the serving thread.
fn start_server(
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
        let (_, path) = parse_request_line(&request_line);
        let (status, body) = route(&path);

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
fn products_list_prints_human_rows() {
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
    assert!(stdout.contains("Brigadeiro"), "unexpected stdout: {stdout}");
    assert!(stdout.contains("R$ 5,00"), "unexpected stdout: {stdout}");
    assert!(stdout.contains("R$ 7,50"), "unexpected stdout: {stdout}");

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
