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
            lines.push(render_products_table(&response.items));
            lines.join("\n")
        },
        format,
    )?;
    println!("{rendered}");
    Ok(())
}

/// Renders a deterministic, aligned table of products.
///
/// Columns are fixed as `ID NAME PRICE CATEGORY AVAILABILITY` and every cell
/// is padded to the widest value in its column, so the layout never depends
/// on the number of results.
pub fn render_products_table(products: &[ProductDto]) -> String {
    const HEADERS: [&str; 5] = ["ID", "NAME", "PRICE", "CATEGORY", "AVAILABILITY"];

    let rows: Vec<[String; 5]> = products
        .iter()
        .map(|product| {
            [
                product.id.clone(),
                product.name.clone(),
                format_brl(product.price),
                product.category_id.clone(),
                availability(product.available).to_string(),
            ]
        })
        .collect();

    let mut widths = [0usize; 5];
    for (index, header) in HEADERS.iter().enumerate() {
        widths[index] = header.len();
    }
    for row in &rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.len());
        }
    }

    let mut lines = Vec::with_capacity(rows.len() + 1);
    lines.push(format_row(&HEADERS.map(str::to_string), &widths));
    for row in rows {
        lines.push(format_row(&row, &widths));
    }
    lines.join("\n")
}

fn format_row(cells: &[String], widths: &[usize]) -> String {
    cells
        .iter()
        .zip(widths)
        .map(|(cell, width)| format!("{cell:<width$}"))
        .collect::<Vec<_>>()
        .join("  ")
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

#[cfg(test)]
mod tests {
    use super::render_products_table;
    use crate::api::ProductDto;

    fn product(id: &str, name: &str, price: u64, category: &str, available: bool) -> ProductDto {
        ProductDto {
            id: id.to_string(),
            name: name.to_string(),
            description: String::new(),
            price,
            category_id: category.to_string(),
            available,
        }
    }

    #[test]
    fn renders_headers_then_products() {
        let products = vec![product(
            "product_brigadeiro",
            "Brigadeiro",
            500,
            "category_snacks",
            true,
        )];
        let table = render_products_table(&products);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 2);

        let header: Vec<&str> = lines[0].split_whitespace().collect();
        assert_eq!(
            header,
            vec!["ID", "NAME", "PRICE", "CATEGORY", "AVAILABILITY"]
        );

        let row: Vec<&str> = lines[1].split_whitespace().collect();
        assert_eq!(
            row,
            vec![
                "product_brigadeiro",
                "Brigadeiro",
                "R$",
                "5,00",
                "category_snacks",
                "available"
            ]
        );
    }

    #[test]
    fn renders_only_the_header_for_an_empty_list() {
        let table = render_products_table(&[]);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("AVAILABILITY"));
    }

    #[test]
    fn marks_unavailable_products() {
        let products = vec![product(
            "product_pastel",
            "Pastel de queijo",
            600,
            "category_snacks",
            false,
        )];
        let table = render_products_table(&products);
        let row: Vec<&str> = table.lines().nth(1).unwrap().split_whitespace().collect();
        assert!(row.contains(&"unavailable"));
        assert!(!row.contains(&"available"));
    }

    #[test]
    fn pads_columns_deterministically() {
        let products = vec![
            product("a", "Short", 300, "cat", true),
            product(
                "very_long_id",
                "A much longer name",
                1250,
                "a_longer_category",
                true,
            ),
        ];
        let table = render_products_table(&products);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines[1].len(), lines[2].len(), "rows must share one width");
        assert!(lines[1].contains("R$ 3,00"));
        assert!(lines[2].contains("R$ 12,50"));
    }
}
