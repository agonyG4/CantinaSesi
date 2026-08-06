//! `cantina auth login`, `auth me`, `auth refresh` and `auth logout`.

use std::io::BufRead;

use rpassword::prompt_password;
use secrecy::{ExposeSecret, SecretString};

use crate::api::{ApiClient, AuthenticationResponse, CliError, UserDto};
use crate::output::{OutputFormat, render};

/// Executes `auth login --email <email>` against the given API base URL.
///
/// The password is read interactively without echo unless `--password-stdin`
/// requests a single line from standard input.
pub async fn login(
    api_url: &str,
    format: OutputFormat,
    email: &str,
    password_stdin: bool,
) -> Result<(), CliError> {
    let password = read_password(email, password_stdin)?;
    let client = ApiClient::new(api_url)?;
    let response = client.login(email, password.expose_secret()).await?;
    print_session(&response, format)
}

/// Executes `auth me` against the given API base URL.
pub async fn me(api_url: &str, format: OutputFormat, token: &SecretString) -> Result<(), CliError> {
    let client = ApiClient::new(api_url)?;
    let user = client.me(token.expose_secret()).await?;
    print_user(&user, format)
}

/// Executes `auth refresh` against the given API base URL.
pub async fn refresh(
    api_url: &str,
    format: OutputFormat,
    token: &SecretString,
) -> Result<(), CliError> {
    let client = ApiClient::new(api_url)?;
    let response = client.refresh(token.expose_secret()).await?;
    print_session(&response, format)
}

/// Executes `auth logout` against the given API base URL.
pub async fn logout(api_url: &str, token: &SecretString) -> Result<(), CliError> {
    let client = ApiClient::new(api_url)?;
    client.logout(token.expose_secret()).await?;
    println!("logged out");
    Ok(())
}

/// Reads the password from a hidden prompt or from a single stdin line.
///
/// The interactive path never echoes; the stdin path reads exactly one line,
/// strips the trailing line ending and fails on an empty line.
fn read_password(email: &str, from_stdin: bool) -> Result<SecretString, CliError> {
    let password = if from_stdin {
        let stdin = std::io::stdin();
        let mut line = String::new();
        stdin
            .lock()
            .read_line(&mut line)
            .map_err(|source| CliError::PasswordRead { source })?;
        line.trim_end_matches(['\r', '\n']).to_string()
    } else {
        prompt_password(format!("password for {email}: "))
            .map_err(|source| CliError::PasswordRead { source })?
    };
    if password.is_empty() {
        return Err(CliError::EmptyPassword);
    }
    Ok(SecretString::from(password))
}

/// Prints an authentication payload in the requested format.
pub fn print_session(
    response: &AuthenticationResponse,
    format: OutputFormat,
) -> Result<(), CliError> {
    let rendered = render(
        response,
        || {
            format!(
                "access_token: {}\ntoken_type: {}\nexpires_at_epoch_seconds: {}\nuser:\n{}",
                response.access_token,
                response.token_type,
                response.expires_at_epoch_seconds,
                render_user_plain(&response.user)
            )
        },
        format,
    )?;
    println!("{rendered}");
    Ok(())
}

/// Prints a single authenticated user in the requested format.
pub fn print_user(user: &UserDto, format: OutputFormat) -> Result<(), CliError> {
    let rendered = render(user, || render_user_plain(user), format)?;
    println!("{rendered}");
    Ok(())
}

/// Renders the user as an aligned `ID NAME EMAIL ROLE STATUS` table.
fn render_user_plain(user: &UserDto) -> String {
    const HEADERS: [&str; 5] = ["ID", "NAME", "EMAIL", "ROLE", "STATUS"];
    let cells: Vec<[&str; 5]> = vec![[
        user.id.as_str(),
        user.name.as_str(),
        user.email.as_str(),
        user.role.as_str(),
        user.status.as_str(),
    ]];

    let mut widths = HEADERS.map(str::len);
    for row in &cells {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.len());
        }
    }

    let mut lines = Vec::with_capacity(cells.len() + 1);
    lines.push(format_row(&HEADERS, &widths));
    for row in cells {
        lines.push(format_row(&row, &widths));
    }
    lines.join("\n")
}

fn format_row(cells: &[&str], widths: &[usize]) -> String {
    cells
        .iter()
        .zip(widths)
        .map(|(cell, width)| format!("{cell:<width$}"))
        .collect::<Vec<_>>()
        .join("  ")
}

#[cfg(test)]
mod tests {
    use super::render_user_plain;
    use crate::api::{AuthenticationResponse, UserDto};

    fn user() -> UserDto {
        UserDto {
            id: "user_student_1".to_string(),
            name: "Ana Silva".to_string(),
            email: "ana@example.com".to_string(),
            role: "student".to_string(),
            status: "active".to_string(),
        }
    }

    #[test]
    fn authentication_response_debug_redacts_the_token() {
        let response = AuthenticationResponse {
            access_token: "known-secret-token-456".to_string(),
            token_type: "Bearer".to_string(),
            expires_at_epoch_seconds: 1893456000,
            user: user(),
        };
        let debug = format!("{response:?}");
        assert!(
            !debug.contains("known-secret-token-456"),
            "Debug must never leak the token: {debug}"
        );
        assert!(
            debug.contains("[REDACTED]"),
            "Debug must redact the token: {debug}"
        );
    }

    #[test]
    fn renders_header_then_user_row() {
        let table = render_user_plain(&user());
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 2);

        let header: Vec<&str> = lines[0].split_whitespace().collect();
        assert_eq!(header, vec!["ID", "NAME", "EMAIL", "ROLE", "STATUS"]);

        let row: Vec<&str> = lines[1].split_whitespace().collect();
        assert_eq!(
            row,
            vec![
                "user_student_1",
                "Ana",
                "Silva",
                "ana@example.com",
                "student",
                "active"
            ]
        );
    }
}
