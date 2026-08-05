//! CantinaSesi administrative CLI.
//!
//! The CLI is an HTTP client of the running `cantina-api`. It never touches
//! the database, never instantiates infrastructure repositories and never
//! bypasses application rules. Everything it knows about the system arrives
//! as structured JSON over HTTP.

pub mod api;
pub mod commands;
pub mod output;

use clap::{Args, Parser, Subcommand};

use crate::output::OutputFormat;

/// Development fallback API base URL.
pub const DEFAULT_API_URL: &str = "http://127.0.0.1:8080";

/// Root command-line interface.
#[derive(Debug, Parser)]
#[command(name = "cantina", version, about = "CantinaSesi administrative client")]
pub struct Cli {
    /// Base URL of the running cantina-api service.
    #[arg(long, global = true)]
    pub api_url: Option<String>,

    /// Output format.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,

    #[command(subcommand)]
    pub command: Command,
}

/// Command tree. Only `health` and `version` are implemented; the rest is
/// prepared for future administrative tasks.
#[derive(Debug, Subcommand)]
pub enum Command {
    Auth(AuthCmd),
    Users(UsersCmd),
    Products(ProductsCmd),
    Orders(OrdersCmd),
    Health,
    Version,
}

#[derive(Debug, Args)]
pub struct AuthCmd {
    #[command(subcommand)]
    pub sub: AuthSub,
}

#[derive(Debug, Subcommand)]
pub enum AuthSub {
    Login,
    Logout,
}

#[derive(Debug, Args)]
pub struct UsersCmd {
    #[command(subcommand)]
    pub sub: UsersSub,
}

#[derive(Debug, Subcommand)]
pub enum UsersSub {
    List,
    Import,
}

#[derive(Debug, Args)]
pub struct ProductsCmd {
    #[command(subcommand)]
    pub sub: ProductsSub,
}

#[derive(Debug, Subcommand)]
pub enum ProductsSub {
    List,
    Create,
    Update,
    Disable,
}

#[derive(Debug, Args)]
pub struct OrdersCmd {
    #[command(subcommand)]
    pub sub: OrdersSub,
}

#[derive(Debug, Subcommand)]
pub enum OrdersSub {
    List,
    Show,
    Confirm,
    Prepare,
    Ready,
    Deliver,
    Cancel,
}

/// Resolves the API base URL.
///
/// Precedence: explicit CLI value, then `CANTINA_API_URL`, then the
/// development default.
pub fn resolve_api_url(cli_value: Option<&str>, env_value: Option<&str>) -> String {
    cli_value
        .or(env_value)
        .unwrap_or(DEFAULT_API_URL)
        .trim_end_matches('/')
        .to_string()
}

/// Executes the given command.
pub async fn run(cli: Cli) -> Result<(), api::CliError> {
    match cli.command {
        Command::Health => {
            let api_url = resolve_api_url(
                cli.api_url.as_deref(),
                std::env::var("CANTINA_API_URL").ok().as_deref(),
            );
            commands::health::run(&api_url, cli.format).await
        }
        Command::Version => {
            commands::version::run();
            Ok(())
        }
        Command::Auth(_) => Err(api::CliError::NotImplemented { command: "auth" }),
        Command::Users(_) => Err(api::CliError::NotImplemented { command: "users" }),
        Command::Products(_) => Err(api::CliError::NotImplemented {
            command: "products",
        }),
        Command::Orders(_) => Err(api::CliError::NotImplemented { command: "orders" }),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_api_url;

    #[test]
    fn cli_flag_wins_over_environment() {
        assert_eq!(
            resolve_api_url(Some("http://example.test:9000"), Some("http://env.test")),
            "http://example.test:9000"
        );
    }

    #[test]
    fn environment_wins_over_default() {
        assert_eq!(
            resolve_api_url(None, Some("http://env.test")),
            "http://env.test"
        );
    }

    #[test]
    fn falls_back_to_development_default() {
        assert_eq!(resolve_api_url(None, None), super::DEFAULT_API_URL);
    }

    #[test]
    fn trims_trailing_slashes() {
        assert_eq!(
            resolve_api_url(Some("http://example.test/"), None),
            "http://example.test"
        );
    }
}
