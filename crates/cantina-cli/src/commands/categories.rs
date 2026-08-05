//! `cantina categories list`.

use crate::api::{ApiClient, CliError};
use crate::output::{OutputFormat, render};

/// Executes `categories list` against the given API base URL.
pub async fn list(api_url: &str, format: OutputFormat) -> Result<(), CliError> {
    let client = ApiClient::new(api_url)?;
    let response = client.list_categories().await?;
    let rendered = render(
        &response,
        || {
            let mut lines = Vec::new();
            lines.push(format!("{} categories", response.items.len()));
            for category in &response.items {
                lines.push(format!("{}   {}", category.name, category.id));
            }
            lines.join("\n")
        },
        format,
    )?;
    println!("{rendered}");
    Ok(())
}
