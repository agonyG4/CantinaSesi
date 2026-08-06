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
existence. The dummy hash is computed at most once per service instance with
that instance's own Argon2 parameters and then reused, so an unknown login does
not generate a fresh salt and hash every time.

### Role-based access control

`Principal::has_role` / `has_any_role` in `cantina-core` express authorization
rules; `AuthorizationService::require_any_role` in `cantina-application`
enforces them and returns a typed `Forbidden`. Roles are server-side only —
clients never supply them.

### Secrets handled by `secrecy::SecretString`

Passwords and access tokens are held as `SecretString` (zeroized on drop) and
never printed, logged, or rendered in error bodies. Structs that must serialize
a token (login/refresh responses) or hold one (the CLI `--token` argument)
still need a `Debug` for ergonomics; instead of deriving it, they implement a
custom redacted `Debug` that prints `[REDACTED]`, so a debug representation can
never reveal a plaintext token or password. `AuthenticationResult` deliberately
drops `PartialEq`/`Eq` because `SecretString` does not implement them.

### Session lifecycle is server-side

Sessions carry an idle TTL (default 30 min) and an absolute TTL (default 8 h),
configurable via `CANTINA_SESSION_IDLE_TTL_SECONDS` /
`CANTINA_SESSION_ABSOLUTE_TTL_SECONDS`. Both timeouts are validated at startup:
zero or inverted values are rejected. `logout` marks the current session
**revoked** (it is rejected immediately; the record may remain in storage until
future cleanup); `refresh` rotates a still-valid session into a brand new one.

Session rotation is **atomic and single-use**. The adapter holds one write lock
while it locates the old session, verifies it exists and is not revoked,
verifies the proposed new fingerprint is unused, then marks the old session
revoked and inserts the new one. Any precondition failure (missing, already
revoked, or a colliding new fingerprint) fails the whole operation and inserts
nothing. A typed `SessionRotationError` distinguishes those cases from genuine
storage failures; the application maps stale or already-used refresh sessions
to the same stable `401` used for invalid access tokens, so a replay or a race
between two refreshes never yields more than one replacement session.

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

## Amendment (2026-08-06): single-use refresh rotation, revoke-only logout

A review found that rotating an unknown, already-revoked, or already-rotated
session still created a new one, allowing two concurrent refreshes of the same
token to both succeed. This amendment tightens the session lifecycle:

* `SessionRepository::rotate_session` is now **atomic** and returns a typed
  `SessionRotationError`. It succeeds only when the old session exists, is not
  revoked, and the proposed new fingerprint is unused; otherwise it fails
  without inserting anything.
* `logout` **revokes** the server-side session instead of deleting it. Revoked
  records may remain until future cleanup, but revoked tokens are rejected
  immediately and never revived.
* The HTTP adapter authenticates each request **once** via an
  `AuthenticatedSession` context (principal plus opaque session fingerprint)
  and hands it to dedicated application methods; refresh/logout no longer
  re-authenticate the raw token.
* The unknown-user dummy verification now reuses one configured dummy Argon2
  hash per service instance instead of hashing a fresh salt on every failed
  login.

This does not alter the original decisions (opaque sessions, Argon2id,
RBAC, in-memory session port).
