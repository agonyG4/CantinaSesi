//! `cantina health` — calls `GET /api/v1/health`.

use crate::api::{ApiClient, CliError, HealthResponse};
use crate::output::{OutputFormat, render};

/// Executes the `health` command against the given API base URL.
pub async fn run(api_url: &str, format: OutputFormat) -> Result<(), CliError> {
    let client = ApiClient::new(api_url.to_string());
    let health = client.health().await?;
    print_health(&health, format);
    Ok(())
}

/// Prints a [`HealthResponse`] in the requested format.
pub fn print_health(health: &HealthResponse, format: OutputFormat) {
    let rendered = render(
        health,
        || {
            format!(
                "status: {}\nservice: {}\nversion: {}",
                health.status, health.service, health.version
            )
        },
        format,
    );
    println!("{rendered}");
}
