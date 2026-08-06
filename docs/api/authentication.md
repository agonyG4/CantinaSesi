# Authentication V1 — opaque session tokens and RBAC

Authentication uses **opaque bearer sessions**. A client logs in once with an
e-mail and password, receives an opaque access token, and presents it in the
`Authorization: Bearer` header on every protected request. Sessions are tracked
server-side and can be revoked.

The contract is described formally in the OpenAPI 3.1 document served at
`GET /api/openapi.json`.

## Roles

Accounts are assigned exactly one role, enforced by the server:

| Role | Meaning |
| --- | --- |
| `student` | Orders from the catalog. |
| `canteen` | Cafeteria staff fulfilling orders. |
| `admin` | System administrator. |

Role values are serialized in lowercase (`student`, `canteen`, `admin`).
Authorization is checked on the server against the authenticated principal;
clients must not trust client-supplied roles.

## Passwords

Passwords are hashed with **Argon2id** and never stored or logged in plaintext.
The API never distinguishes "unknown account" from "wrong password" — both
return the same `401 INVALID_CREDENTIALS` so attackers cannot enumerate
accounts. The login endpoint also runs a dummy Argon2 verification when a hash
is absent, keeping response timing uniform.

## Endpoints

### `POST /api/v1/auth/login`

Authenticates with e-mail and password.

```bash
curl -s http://127.0.0.1:8080/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"ana@example.com","password":"StudentDemo!2026"}'
```

E-mail is normalized (trimmed and lowercased) before lookup. A request with a
blank e-mail or password is a `400 INVALID_REQUEST_BODY`.

Response `200` (also sent by `refresh`):

```json
{
  "access_token": "<opaque session token>",
  "token_type": "Bearer",
  "expires_at_epoch_seconds": 1780000000,
  "user": {
    "id": "user_student_1",
    "name": "Ana Silva",
    "email": "ana@example.com",
    "role": "student",
    "status": "active"
  }
}
```

Responses carry `Cache-Control: no-store` and `Pragma: no-cache` so clients
never cache credentials. The access token is returned exactly once and is never
logged.

| Error | HTTP | Meaning |
| --- | --- | --- |
| `INVALID_REQUEST_BODY` | `400` | Missing/blank e-mail or password, or an oversized body. |
| `INVALID_CREDENTIALS` | `401` | Unknown e-mail or wrong password. |
| `ACCOUNT_INACTIVE` | `403` | The account exists and the password is correct, but the account is disabled. |

### `POST /api/v1/auth/refresh`

Issues a new session from a currently valid bearer token, so long-lived clients
can rotate tokens. Requires `Authorization: Bearer <token>`.

Returns the same `200` payload as `login`.

### `POST /api/v1/auth/logout`

Revokes the current session. Requires `Authorization: Bearer <token>`.

Response `204` with no body. The presented token is immediately invalid.

### `GET /api/v1/auth/me`

Returns the authenticated user. Requires `Authorization: Bearer <token>`.

Response `200`:

```json
{
  "id": "user_student_1",
  "name": "Ana Silva",
  "email": "ana@example.com",
  "role": "student",
  "status": "active"
}
```

## Using a token

Protected endpoints read the token from the `Authorization` header:

```bash
TOKEN=$(curl -s http://127.0.0.1:8080/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"ana@example.com","password":"StudentDemo!2026"}' \
  | python -c 'import sys,json; print(json.load(sys.stdin)["access_token"])')

curl -s http://127.0.0.1:8080/api/v1/auth/me \
  -H "Authorization: Bearer $TOKEN"
```

The `Bearer` scheme is matched case-insensitively. A missing, malformed or
blank header is an `AUTHENTICATION_REQUIRED` challenge.

## Session semantics

Sessions are opaque server-side records identified by a 32-byte random token
whose **SHA-256 fingerprint** is stored. The plaintext token is never stored or
logged, so a database leak cannot be replayed as a valid session.

Two server-side timeouts bound every session:

| Variable | Default | Meaning |
| --- | --- | --- |
| `CANTINA_SESSION_IDLE_TTL_SECONDS` | `1800` (30 min) | A session expires if unused for this long. |
| `CANTINA_SESSION_ABSOLUTE_TTL_SECONDS` | `28800` (8 h) | A session expires this long after creation, regardless of activity. |

Zero or inverted values (idle exceeding absolute) are rejected at startup with
a typed configuration error.

## Errors

Authentication failures always include a `WWW-Authenticate: Bearer` header and
use the standardized envelope:

```json
{
  "error": {
    "code": "INVALID_ACCESS_TOKEN",
    "message": "the access token is invalid",
    "request_id": "3b3..."
  }
}
```

| Code | HTTP | Meaning |
| --- | --- | --- |
| `AUTHENTICATION_REQUIRED` | `401` | No valid bearer token presented. |
| `INVALID_ACCESS_TOKEN` | `401` | Unknown or revoked token. |
| `SESSION_EXPIRED` | `401` | The session exceeded its idle/absolute timeout. |
| `ACCOUNT_INACTIVE` | `403` | The account is disabled. |
| `FORBIDDEN` | `403` | The principal lacks the required role. |

## Demo users

The in-memory seed creates four demo accounts (development only, one-way
Argon2id hashes):

| E-mail | Password | Role | Status |
| --- | --- | --- | --- |
| `ana@example.com` | `StudentDemo!2026` | student | active |
| `cantina@example.com` | `CanteenDemo!2026` | canteen | active |
| `admin@example.com` | `AdminDemo!2026` | admin | active |
| `inactive@example.com` | `InactiveDemo!2026` | student | inactive |

These accounts exist only to exercise the API; production data comes from the
real database adapter once it lands.

## Administrative CLI

The CLI mirrors this contract. `login` reads the password from a hidden prompt,
or from standard input with `--password-stdin`:

```bash
cargo run -p cantina-cli -- auth login --email ana@example.com
cargo run -p cantina-cli -- auth login --email ana@example.com --password-stdin

# authenticated commands accept --token or CANTINA_ACCESS_TOKEN
cargo run -p cantina-cli -- --token "$TOKEN" auth me
cargo run -p cantina-cli -- --token "$TOKEN" auth refresh
cargo run -p cantina-cli -- --token "$TOKEN" auth logout
export CANTINA_ACCESS_TOKEN="$TOKEN"
cargo run -p cantina-cli -- auth me
```

`auth me` renders an `ID NAME EMAIL ROLE STATUS` table in human format. The
token is held in memory only and never persisted by the CLI.
