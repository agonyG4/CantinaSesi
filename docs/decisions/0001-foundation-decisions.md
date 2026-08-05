# ADR 0001: Foundation decisions

- Status: accepted
- Date: 2026-08-05

## Context

The backend foundation needs a few representation choices that ripple through
every crate. This record keeps them explicit so they are not re-litigated on
every change.

## Decision

### Strongly typed identifiers

All entity identifiers are strongly typed newtypes over `String`:
`UserId`, `ProductId`, `CategoryId`, `OrderId`. Raw integers and bare strings
are never used as identifiers in domain code. The string form (`order_123`)
is opaque; consumers must not parse it.

### Money as integer minor units

Prices and order totals are `u64` counts of minor units (centavos), never
floating point. This avoids rounding errors. The HTTP contract exposes the
same integer representation; formatting into a decimal string (for example
`18.50`) is a presentation concern and is deliberately not part of the API
contract yet.

### Domain crate is dependency-free

`cantina-core` has zero external dependencies. Errors are plain
`Display`/`Error` implementations; no `thiserror`, no serde. This makes the
"framework-independent domain" property verifiable by inspection.

### Ports are owned by the application layer

Repository and service interfaces live in `cantina-application::ports` and
are described in domain terms. Adapters in `cantina-infrastructure` implement
them. The reverse is forbidden.

### No database yet

The foundation ships in-memory repository adapters only, for startup, tests
and dependency-injection demonstration. Picking a database, migrations and an
ORM is a separate, later decision confined to `cantina-infrastructure`.

### No real authentication yet

Passwords, JWT and sessions are out of scope. The domain exposes
`Principal` and `AuthError`; the application exposes `PasswordVerifier`,
`TokenIssuer` and an `AuthenticateUser` contract that returns a typed
"not implemented" error.

## Consequences

- Replacing the UI requires no domain or application changes.
- Replacing the storage requires changing only adapters and the composition
  root.
- Tests exercise the exact same ports production will use.
