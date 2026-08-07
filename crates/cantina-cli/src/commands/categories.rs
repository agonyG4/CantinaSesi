//! `cantina categories` command implementations.

use secrecy::ExposeSecret;

use crate::api::{ApiClient, CategoryDto, CliError, CreateCategoryRequest, UpdateCategoryRequest};
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

/// Executes `categories create` against the given API base URL.
pub async fn create(
    api_url: &str,
    format: OutputFormat,
    token: &secrecy::SecretString,
    request: &CreateCategoryRequest,
) -> Result<(), CliError> {
    let client = ApiClient::new(api_url)?;
    let category = client
        .create_category(token.expose_secret(), request)
        .await?;
    print_category(&category, format)
}

/// Executes `categories update <category-id>` against the given API base URL.
pub async fn update(
    api_url: &str,
    format: OutputFormat,
    token: &secrecy::SecretString,
    category_id: &str,
    request: &UpdateCategoryRequest,
) -> Result<(), CliError> {
    let client = ApiClient::new(api_url)?;
    let category = client
        .update_category(token.expose_secret(), category_id, request)
        .await?;
    print_category(&category, format)
}

/// Prints a single [`CategoryDto`] in the requested format.
pub fn print_category(category: &CategoryDto, format: OutputFormat) -> Result<(), CliError> {
    let rendered = render_category(category, format)?;
    println!("{rendered}");
    Ok(())
}

/// Renders a single [`CategoryDto`] in the requested format.
pub fn render_category(category: &CategoryDto, format: OutputFormat) -> Result<String, CliError> {
    render(
        category,
        || format!("ID: {}\nNAME: {}", category.id, category.name),
        format,
    )
}

#[cfg(test)]
mod tests {
    use super::render_category;
    use crate::api::CategoryDto;
    use crate::output::OutputFormat;

    fn category() -> CategoryDto {
        CategoryDto {
            id: "category_snacks".to_string(),
            name: "Snacks".to_string(),
        }
    }

    #[test]
    fn human_output_prints_id_then_name() {
        let rendered = render_category(&category(), OutputFormat::Human).unwrap();
        assert!(rendered.contains("ID: category_snacks"));
        assert!(rendered.contains("NAME: Snacks"));
    }

    #[test]
    fn json_output_prints_the_exact_payload() {
        let rendered = render_category(&category(), OutputFormat::Json).unwrap();
        let json: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(json["id"], "category_snacks");
        assert_eq!(json["name"], "Snacks");
    }
}
