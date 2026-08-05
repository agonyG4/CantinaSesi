//! `cantina version` — prints the CLI version.

/// Executes the `version` command.
pub fn run() {
    println!("cantina {}", env!("CARGO_PKG_VERSION"));
}
