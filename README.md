# CantinaSesi

Sabor Digital

CantinaSesi is a web platform for the SESI/SENAI school cafeteria. The entire
backend is written in **Rust** and follows a ports-and-adapters
(hexagonal) architecture: the API and the CLI are separate adapters that both
speak the same HTTP contract, so the frontend can be replaced at any time
without touching business rules.

## Workspace

```text
crates/
├── cantina-core/          Pure business concepts (no framework, no I/O)
├── cantina-application/   Use cases, DTOs, ports, typed errors
├── cantina-infrastructure/ Replaceable adapters (config, in-memory storage, ids, clock)
├── cantina-api/           HTTP adapter (Axum) and composition root
└── cantina-cli/           Administrative HTTP client (Clap + Reqwest)
docs/
├── architecture/backend.md
└── decisions/
```

## Commands

```bash
cargo build --workspace
cargo run -p cantina-api
cargo run -p cantina-cli -- health
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## How the pieces fit together

Rust powers the whole backend. The HTTP API (`cantina-api`) composes the
application services with the infrastructure adapters; the CLI
(`cantina-cli`) is a thin administrative client that talks to the running API
over HTTP — it never touches the database. Every future frontend (web, mobile,
desktop) connects to the same API contract.

See `docs/architecture/backend.md` for the full architecture.
