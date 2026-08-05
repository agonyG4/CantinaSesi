//! `cantina products list` and `cantina products show`.

use crate::api::{ApiClient, CliError, ProductDto, ProductsQuery};
use crate::output::{OutputFormat, format_brl, render};

/// Executes `products list` against the given API base URL.
pub async fn list(
    api_url: &str,
    format: OutputFormat,
    query: &ProductsQuery,
) -> Result<(), CliError> {
    let client = ApiClient::new(api_url)?;
    let response = client.list_products(query).await?;
    let rendered = render(
        &response,
        || {
            let mut lines = Vec::new();
            lines.push(format!("{} products", response.items.len()));
            for product in &response.items {
                lines.push(format!(
                    "{}   {}   [{}]   {}",
                    product.name,
                    format_brl(product.price),
                    product.category_id,
                    availability(product.available)
                ));
            }
            lines.join("\n")
        },
        format,
    )?;
    println!("{rendered}");
    Ok(())
}

/// Executes `products show <id>` against the given API base URL.
pub async fn show(api_url: &str, format: OutputFormat, id: &str) -> Result<(), CliError> {
    let client = ApiClient::new(api_url)?;
    let product = client.get_product(id).await?;
    print_product(&product, format)
}

/// Prints a single [`ProductDto`] in the requested format.
pub fn print_product(product: &ProductDto, format: OutputFormat) -> Result<(), CliError> {
    let rendered = render(
        product,
        || {
            format!(
                "name: {}\nid: {}\ndescription: {}\nprice: {}\ncategory: {}\navailable: {}",
                product.name,
                product.id,
                product.description,
                format_brl(product.price),
                product.category_id,
                availability(product.available)
            )
        },
        format,
    )?;
    println!("{rendered}");
    Ok(())
}

fn availability(available: bool) -> &'static str {
    if available {
        "available"
    } else {
        "unavailable"
    }
}
