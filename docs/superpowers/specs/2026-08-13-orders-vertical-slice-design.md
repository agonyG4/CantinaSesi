# Orders Vertical Slice Design

**Date:** 2026-08-13

**Goal:** Make the Orders vertical slice usable by the cafeteria MVP while moving all order authorization into the application layer and preserving the existing hexagonal architecture and in-memory persistence.

## Context and decisions

The repository already separates `cantina-core`, `cantina-application`, `cantina-infrastructure`, and `cantina-api`. Authentication resolves opaque sessions into a domain `Principal` containing a `UserId` and `UserRole`. The existing order service is incomplete: it accepts a client-supplied student id, performs one product lookup per line, exposes an unauthorised order route, and has no cafeteria routes.

The implementation will keep the existing status lifecycle (`pending -> confirmed -> preparing -> ready -> delivered`, with cancellation only from pending or confirmed) and will not add payment, inventory, notifications, or PostgreSQL. `OrderId` remains an opaque string for compatibility; authorization is the primary protection against IDOR, while the existing generator may be hardened only if doing so does not create unrelated migration churn.

## Architecture

### Application authorization

Every authenticated order use case receives `&Principal`. `OrderService` owns the policy:

- students may create orders for `principal.user_id()`, list their own history, and retrieve their own orders;
- canteen and admin principals may retrieve orders, list the active cafeteria queue, and apply valid domain status transitions;
- student access to another student’s order returns the project’s non-enumerating not-found result;
- role failures return the existing `Forbidden` application error.

The create command contains only item inputs. Ownership is never accepted from HTTP or from the application command.

### Trusted order construction

The service validates centralized order limits, consolidates duplicate product ids deterministically, fetches all unique products through one `find_products_by_ids` port operation, rejects missing or unavailable products, constructs item snapshots from trusted catalog values, performs checked total arithmetic, generates a pickup code, and inserts the order through an atomic repository operation that enforces pickup-code uniqueness.

The order domain will expose checked total calculation and a typed overflow error. The existing order invariant will ensure that persisted orders cannot contain an overflowing total; callers will not observe a panic or wrapping arithmetic.

### Pickup codes and concurrency

Production pickup codes will use the operating-system CSPRNG with the existing human-friendly alphabet and a six-character length. The application will retry a bounded number of times when the repository reports a pickup-code conflict. The new-order repository operation performs uniqueness checking and insertion while holding the adapter’s existing mutex, eliminating the check-then-insert race and providing a future SQL adapter with a clear unique-constraint boundary.

Tests will inject deterministic generators and a repository that reports controlled collisions; no production randomness will be used to make tests deterministic.

### HTTP and visibility

The API will expose:

```text
POST   /api/v1/orders
GET    /api/v1/orders
GET    /api/v1/orders/{order_id}
GET    /api/v1/canteen/orders
PATCH  /api/v1/canteen/orders/{order_id}/status
```

Handlers will only parse transport data, extract `AuthenticatedSession`, invoke application methods, and map DTOs/errors. Student and cafeteria routes remain separate. Responses are built from authorized application views; pickup codes are available only after the service has authorized the caller. Student requests never accept `student_id`, price, total, pickup code, or initial status.

The request body has a bounded size. Order limits are centralized: at least one line, a small maximum number of distinct lines, quantity greater than zero, a maximum quantity per product, and a maximum total quantity. Duplicate lines are summed with checked quantity arithmetic and then validated against those limits.

### Errors and OpenAPI

New stable application errors cover invalid quantities, line and quantity limits, missing/unavailable products, total overflow, pickup-code retry exhaustion, unauthorized order operations, and invalid status transitions. The HTTP adapter maps these to the existing structured error envelope and appropriate `400`, `401`, `403`, `404`, `409`, `422`, and `500` statuses without leaking storage details or another student’s order existence.

The handwritten `utoipa` document will include all order paths, request/response schemas, status values, security requirements, query parameters if implemented, and shared error responses. OpenAPI integration tests will assert that the complete route surface and key schemas remain documented.

## Testing strategy

Application/domain tests will cover the authorization matrix, ownership, bulk product lookup count, duplicate normalization, centralized limits, checked totals and overflow, valid/invalid transitions, random-generator contract, collision retry, and retry exhaustion. Router-level tests will cover student creation/history/detail, cross-student denial and pickup-code non-leakage, cafeteria listing/detail/status updates, role denial, unauthenticated requests, invalid payloads, unavailable products, limits, and duplicate lines. Existing health, auth, catalog, CLI, and OpenAPI tests must remain green.

## Scope boundary

This change intentionally does not add PostgreSQL/SQLx, payments, inventory reservation, WebSockets, notifications, frontend code, Redis, distributed locks, event sourcing, or unrelated identifier migrations.
