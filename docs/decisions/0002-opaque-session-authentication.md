# ADR 0002: Opaque session authentication with role-based access control

- Status: accepted
- Date: 2026-08-06

## Context

The foundation (ADR 0001) shipped `Principal`, `AuthError` and placeholder
`PasswordVerifier`/`TokenIssuer` contracts that returned "not implemented".
Authentication and authorization are now the first security-sensitive vertical
slice. Two families of schemes were considered:

* **Stateless JWT**: a signed token carries claims and needs no server-side
  record. Cheap to verify, but revocation is hard and token size grows with
  claims.
* **Opaque server-side sessions**: a random token is a lookup key into a
  session record. Revocation is trivial (delete the record), and the token
  reveals nothing on its own.

A school cafeteria needs: instant logout, simple per-account revocation, and no
token replay after a database leak. These favor an opaque, server-side session.

## Decision

### Opaque bearer sessions

A login issues an opaque access token — 32 bytes from the OS CSPRNG, URL-safe
unpadded Base64. The server stores only the token's **SHA-256 fingerprint**
next to the session record; the plaintext token is never stored or logged.
Authorization for `Authorization: Bearer <token>` fingerprints the presented
token and looks it up.

### Argon2id password hashing

Passwords are hashed with Argon2id v19 (PHC format, 19 MiB memory cost,
time cost 2, parallelism 1) via RustCrypto's `argon2`. All hashing/verifying
runs on a blocking thread pool (`spawn_blocking`) to avoid stalling the async
runtime. Tests use low-cost parameters; production uses the documented costs.

### Unknown-account and wrong-password are indistinguishable

Both return `401 INVALID_CREDENTIALS`. When no hash exists for an e-mail, a
dummy Argon2 verification runs so response timing does not leak account
existence.

### Role-based access control

`Principal::has_role` / `has_any_role` in `cantina-core` express authorization
rules; `AuthorizationService::require_any_role` in `cantina-application`
enforces them and returns a typed `Forbidden`. Roles are server-side only —
clients never supply them.

### Secrets handled by `secrecy::SecretString`

Passwords and access tokens are held as `SecretString` (zeroized on drop) and
never printed, logged, or rendered in error bodies. Deriving `Debug` that would
reveal a secret is avoided; `AuthenticationResult` deliberately drops
`PartialEq`/`Eq` because `SecretString` does not implement them.

### Session lifecycle is server-side

Sessions carry an idle TTL (default 30 min) and an absolute TTL (default 8 h),
configurable via `CANTINA_SESSION_IDLE_TTL_SECONDS` /
`CANTINA_SESSION_ABSOLUTE_TTL_SECONDS`. Both timeouts are validated at startup:
zero or inverted values are rejected. `logout` deletes the record; `refresh`
issues a new session from a still-valid one.

### CLI never persists tokens

`--token` (or `CANTINA_ACCESS_TOKEN`) is held in memory for one invocation only
and never written to disk. Interactive `auth login` reads the password via a
hidden prompt; `--password-stdin` reads a single line and fails on empty input.

## Alternatives considered

* **JWT**: rejected — revocation requires a blacklist or short expiries, and
  the token itself cannot be invalidated without server state, which defeats
  the point of going stateless.
* **Plain or fast-hashed passwords**: rejected — Argon2id is the memory-hard
  standard for password storage.
* **Client-supplied roles / claims**: rejected — authorization must be
  server-side and sourced from the account record.

## Consequences

- Logout and revocation are immediate and free.
- A database leak yields fingerprints, not usable tokens, and no plaintext
  passwords.
- Protected HTTP endpoints now require the bearer challenge, and authorization
  rules can be applied to any endpoint via `require_any_role`.
- Verification timing is kept uniform against account enumeration.
- Session storage is in-memory today; the port (`SessionRepository`) is stable,
  so a real database adapter can replace it without changing the application.
