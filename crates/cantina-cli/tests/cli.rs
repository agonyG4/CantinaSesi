//! CLI integration tests that exercise the real binary over HTTP.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_cantina-cli");

/// Starts a tiny HTTP server answering `/api/v1/health` with a healthy body.
/// Returns the base URL and the serving thread.
fn start_health_server() -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();

    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0u8; 2048];
        let _ = stream.read(&mut buffer);

        let body = r#"{"status":"healthy","service":"cantina-api","version":"0.1.0"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });

    (format!("http://{address}"), handle)
}

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
    let (url, server) = start_health_server();

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
