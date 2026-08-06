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

    /// Access token for authenticated commands.
    ///
    /// Takes precedence over `CANTINA_ACCESS_TOKEN`. The value is held in
    /// memory only and never persisted.
    #[arg(long, global = true)]
    pub token: Option<String>,

    /// Output format.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,

    #[command(subcommand)]
    pub command: Command,
}

/// Command tree. `health`, `version`, `products` and `categories` are
/// implemented; the rest is prepared for future administrative tasks.
#[derive(Debug, Subcommand)]
pub enum Command {
    Auth(AuthCmd),
    Users(UsersCmd),
    Products(ProductsCmd),
    Categories(CategoriesCmd),
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
    /// Authenticate and print a new access token.
    Login(AuthLoginArgs),
    /// Print the authenticated user.
    Me,
    /// Issue a new session from the current access token.
    Refresh,
    /// Revoke the current session.
    Logout,
}

/// Arguments accepted by `auth login`.
#[derive(Debug, Args)]
pub struct AuthLoginArgs {
    /// E-mail address of the account.
    #[arg(long)]
    pub email: String,

    /// Read the password from standard input instead of prompting.
    ///
    /// Reads exactly one line, strips the trailing line ending and fails on
    /// an empty line. The value is never echoed.
    #[arg(long)]
    pub password_stdin: bool,
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
    List(ProductsListArgs),
    Show(ProductShowArgs),
    Create,
    Update,
    Disable,
}

/// Filters accepted by `products list`.
#[derive(Debug, Args)]
pub struct ProductsListArgs {
    /// Restrict the results to a single category.
    #[arg(long)]
    pub category_id: Option<String>,

    /// Case-insensitive substring match against the name or description.
    #[arg(long)]
    pub search: Option<String>,

    /// Include unavailable products in the listing.
    #[arg(long, visible_alias = "include-unavailable")]
    pub all: bool,
}

/// Identifier accepted by `products show`.
#[derive(Debug, Args)]
pub struct ProductShowArgs {
    /// Product identifier.
    pub id: String,
}

#[derive(Debug, Args)]
pub struct CategoriesCmd {
    #[command(subcommand)]
    pub sub: CategoriesSub,
}

#[derive(Debug, Subcommand)]
pub enum CategoriesSub {
    List,
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

/// Resolves the API base URL from the CLI flag and environment.
fn api_url(cli_value: Option<&str>) -> String {
    resolve_api_url(cli_value, std::env::var("CANTINA_API_URL").ok().as_deref())
}

/// Resolves the access token from the CLI flag and environment.
///
/// Precedence: explicit `--token`, then `CANTINA_ACCESS_TOKEN`. The token is
/// held in memory only and never persisted.
fn access_token(cli_value: Option<&str>) -> Result<secrecy::SecretString, api::CliError> {
    match cli_value {
        Some(value) if !value.is_empty() => Ok(secrecy::SecretString::from(value.to_string())),
        Some(_) => Err(api::CliError::MissingAccessToken),
        None => match std::env::var("CANTINA_ACCESS_TOKEN") {
            Ok(value) if !value.is_empty() => Ok(secrecy::SecretString::from(value)),
            _ => Err(api::CliError::MissingAccessToken),
        },
    }
}

/// Executes the given command.
pub async fn run(cli: Cli) -> Result<(), api::CliError> {
    match cli.command {
        Command::Health => {
            commands::health::run(&api_url(cli.api_url.as_deref()), cli.format).await
        }
        Command::Version => {
            commands::version::run();
            Ok(())
        }
        Command::Products(cmd) => match cmd.sub {
            ProductsSub::List(args) => {
                let query = api::ProductsQuery {
                    category_id: args.category_id,
                    search: args.search,
                    include_unavailable: args.all,
                };
                commands::products::list(&api_url(cli.api_url.as_deref()), cli.format, &query).await
            }
            ProductsSub::Show(args) => {
                commands::products::show(&api_url(cli.api_url.as_deref()), cli.format, &args.id)
                    .await
            }
            ProductsSub::Create => Err(api::CliError::NotImplemented {
                command: "products create",
            }),
            ProductsSub::Update => Err(api::CliError::NotImplemented {
                command: "products update",
            }),
            ProductsSub::Disable => Err(api::CliError::NotImplemented {
                command: "products disable",
            }),
        },
        Command::Categories(cmd) => match cmd.sub {
            CategoriesSub::List => {
                commands::categories::list(&api_url(cli.api_url.as_deref()), cli.format).await
            }
        },
        Command::Auth(cmd) => match cmd.sub {
            AuthSub::Login(args) => {
                commands::auth::login(
                    &api_url(cli.api_url.as_deref()),
                    cli.format,
                    &args.email,
                    args.password_stdin,
                )
                .await
            }
            AuthSub::Me => {
                let token = access_token(cli.token.as_deref())?;
                commands::auth::me(&api_url(cli.api_url.as_deref()), cli.format, &token).await
            }
            AuthSub::Refresh => {
                let token = access_token(cli.token.as_deref())?;
                commands::auth::refresh(&api_url(cli.api_url.as_deref()), cli.format, &token).await
            }
            AuthSub::Logout => {
                let token = access_token(cli.token.as_deref())?;
                commands::auth::logout(&api_url(cli.api_url.as_deref()), &token).await
            }
        },
        Command::Users(_) => Err(api::CliError::NotImplemented { command: "users" }),
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
