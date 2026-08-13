# Secure Orders Vertical Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a secure, tested, documented Orders vertical slice for the cafeteria MVP while retaining the current in-memory adapters and layered Rust/Axum architecture.

**Architecture:** `OrderService` will receive `&Principal` for every authenticated operation and will own role/ownership policy. The repository ports will expose bulk product lookup and an atomic new-order insert that enforces pickup-code uniqueness; in-memory adapters will implement both with short synchronous mutex scopes. HTTP handlers will remain transport adapters and will use the existing session extractor, structured errors, and utoipa document.

**Tech Stack:** Rust 1.97, Axum 0.8, Tokio, async-trait, serde, utoipa 5, getrandom/OS CSPRNG, existing in-memory repositories and test utilities.

## Global Constraints

- Do not add PostgreSQL, SQLx, payment gateway integration, inventory reservation, WebSockets, frontend, notifications, analytics, Redis, distributed locks, event sourcing, or microservices.
- Preserve health, authentication/session, catalog, RBAC, CLI, existing OpenAPI contracts, and in-memory persistence.
- Never trust client-provided `student_id`, price, total, pickup code, or initial status.
- Keep authorization in `cantina-application`; keep HTTP handlers thin.
- Use checked multiplication/addition for order totals and duplicate quantities.
- Use a bounded pickup-code collision retry count and an atomic repository boundary.
- Do not hold a synchronous mutex across `.await`.
- Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` before completion.

---

### Task 1: Make order invariants and limits explicit

**Files:**
- Modify: `crates/cantina-core/src/orders.rs`
- Modify: `crates/cantina-application/src/error.rs`
- Test: `crates/cantina-core/src/orders.rs` and `crates/cantina-application/src/order_tests.rs`

**Interfaces:**
- Produce `MAX_ORDER_LINES`, `MAX_QUANTITY_PER_PRODUCT`, `MAX_TOTAL_QUANTITY`, `MAX_PICKUP_CODE_ATTEMPTS`, `OrderError::TotalOverflow`, and checked order total behavior for later application code.
- Preserve the existing lifecycle and all valid transitions.

- [ ] Add failing domain tests for `u64::MAX * 2`, `u64::MAX + 1`, and valid totals.
- [ ] Run `cargo test -p cantina-core orders::tests::` and confirm the new overflow assertions fail because the current implementation wraps/panics or lacks the typed error.
- [ ] Add checked subtotal/accumulation using `checked_mul` and `checked_add`; return `OrderError::TotalOverflow` from construction or the checked total API so no persisted order can overflow.
- [ ] Add application mappings for total overflow and centralized limit failures with stable variants/messages.
- [ ] Run the focused core/application tests and confirm the new tests pass.
- [ ] Commit `feat(orders): enforce order invariants and limits`.

### Task 2: Add bulk catalog lookup and atomic order persistence ports

**Files:**
- Modify: `crates/cantina-application/src/ports.rs`
- Modify: `crates/cantina-infrastructure/src/persistence/in_memory.rs`
- Modify: `crates/cantina-application/src/test_support.rs`
- Test: repository tests in those files and `crates/cantina-application/src/order_tests.rs`

**Interfaces:**
- Add `ProductRepository::find_products_by_ids(&[ProductId]) -> Result<Vec<Product>, RepositoryError>`.
- Add an order write result distinguishing pickup-code collision from storage failure, and add `OrderRepository::save_new_order(&Order) -> Result<(), OrderWriteError>`.
- Keep `save_order` for updates, with the new insert operation enforcing both order-id and pickup-code uniqueness atomically.

- [ ] Add a counting product repository test double and a failing test asserting one bulk call for repeated/multiple product ids.
- [ ] Add a failing in-memory repository test showing a duplicate pickup code is rejected without replacing the existing order.
- [ ] Implement the ports and adapters; bulk lookup must acquire the product mutex once, and atomic order insert must check id/code and insert while holding one mutex guard.
- [ ] Sort repository list results deterministically by `created_at` descending and id as a stable tie-breaker where the adapter can do so without changing unrelated catalog behavior.
- [ ] Run focused infrastructure/application tests and confirm pass.
- [ ] Commit `feat(orders): add bulk lookup and atomic order writes`.

### Task 3: Harden pickup-code and order-id generation

**Files:**
- Modify: `crates/cantina-infrastructure/src/identifiers.rs`
- Modify: `crates/cantina-application/src/ports.rs` only if the generator error contract needs extension
- Test: `crates/cantina-infrastructure/src/identifiers.rs`

**Interfaces:**
- Keep `PickupCodeGenerator` injectable for deterministic tests.
- Production `DefaultPickupCodeGenerator` must use OS randomness, the human-friendly alphabet, and six characters; it must not use `AtomicU64` or timestamps.

- [ ] Add a test that samples codes and verifies length/alphabet and non-sequential behavior without asserting a probabilistic collision.
- [ ] Run the focused infrastructure identifier tests before replacing the generator and record the expected failure for the new six-character/CSPRNG contract.
- [ ] Implement generation with `getrandom` and retain a small, readable alphabet without ambiguous characters; map an OS RNG failure to a safe fallback only if the port remains infallible, or change the port to a typed error and propagate it through application errors.
- [ ] Keep the existing `Generator` compatibility behavior unless a small random order-id change is required by the public-route threat model; document the final choice.
- [ ] Run identifier tests and clippy for the infrastructure crate.
- [ ] Commit `fix(orders): make pickup codes cryptographically random`.

### Task 4: Move order authorization and construction into `OrderService`

**Files:**
- Modify: `crates/cantina-application/src/orders.rs`
- Modify: `crates/cantina-application/src/error.rs`
- Modify: `crates/cantina-application/src/order_tests.rs`
- Modify: `crates/cantina-application/src/test_support.rs`

**Interfaces:**
- Change `CreateOrderCommand` to contain only `items`.
- Implement `create_order(&self, principal: &Principal, command: CreateOrderCommand)`.
- Implement `get_order(&self, principal: &Principal, id: OrderId)`, `list_student_orders(&self, principal: &Principal)`, `list_canteen_orders(&self, principal: &Principal)`, and `change_status(&self, principal: &Principal, id: OrderId, target: OrderStatus)`.
- Keep `OrderDto` as a trusted application view; all callers must pass service authorization before receiving it.

- [ ] Update application tests first to construct principals and assert students cannot create as another identity, access another student’s order, list the canteen queue, or change status; assert canteen/admin access and valid transition behavior.
- [ ] Run the focused application tests and confirm compile/test failures identify the missing principal-aware APIs.
- [ ] Implement role checks with `AuthorizationService`, derive the student id only from `principal.user_id()`, and use a non-enumerating not-found error for cross-student reads.
- [ ] Normalize duplicate item ids in first-seen order with checked quantity addition; enforce at least one line, maximum distinct lines, per-product quantity, and total quantity before catalog access.
- [ ] Use one bulk product lookup, reject any missing/unavailable product without revealing unrelated catalog details, build trusted snapshots, and call `save_new_order` with at most `MAX_PICKUP_CODE_ATTEMPTS` retries on collision.
- [ ] Map domain, repository, overflow, collision exhaustion, and authorization failures to stable `ApplicationError` variants.
- [ ] Run all application tests and refactor only while green.
- [ ] Commit `feat(orders): authorize and construct orders in application layer`.

### Task 5: Implement student and canteen HTTP routes

**Files:**
- Modify: `crates/cantina-api/src/routes/orders.rs`
- Modify: `crates/cantina-api/src/routes/canteen.rs`
- Modify: `crates/cantina-api/src/routes/mod.rs`
- Modify: `crates/cantina-api/src/error.rs`
- Test: `crates/cantina-api/tests/http.rs`

**Interfaces:**
- Add `POST /api/v1/orders`, `GET /api/v1/orders`, `GET /api/v1/orders/{order_id}`.
- Add `GET /api/v1/canteen/orders` and `PATCH /api/v1/canteen/orders/{order_id}/status`.
- Use `AuthenticatedSession`, `RequestId`, and existing `ApiError` mapping in every protected handler.

- [ ] Add router tests for valid student creation, own history/detail, cross-student 404 with no pickup code, student denial of queue/status, unauthenticated denial, canteen/admin queue/detail, valid status transition, invalid transition, unavailable/missing products, limits, and duplicate lines.
- [ ] Run the new HTTP tests before implementation and confirm route-not-found or compile failures.
- [ ] Add serde/utoipa request types containing only product ids and quantities; reject malformed/oversized bodies through the existing body-limit pattern.
- [ ] Keep student and cafeteria routers separate; handlers only translate JSON/path/query data and pass `context.principal` to the service.
- [ ] Return `201 Created` for creation with a `Location` header, `200 OK` for reads/patches, and the existing structured errors for failures.
- [ ] Ensure response serialization does not expose another student’s order or pickup code and does not accept trusted business fields from clients.
- [ ] Run all API tests, including all pre-existing catalog/auth tests.
- [ ] Commit `feat(api): expose secure order workflow routes`.

### Task 6: Document the Orders surface in OpenAPI

**Files:**
- Modify: `crates/cantina-api/src/routes/orders.rs`
- Modify: `crates/cantina-api/src/routes/canteen.rs`
- Modify: `crates/cantina-api/src/routes/openapi.rs`
- Modify: `crates/cantina-api/tests/http.rs`

**Interfaces:**
- Add utoipa path metadata for all five order endpoints, order request/item/response schemas, order status values, bearer security, success codes, and relevant shared errors.

- [ ] Add OpenAPI tests asserting every order path exists, POST has its request schema and 201 response, PATCH documents status request/response, security is present, and shared error responses cover 400/401/403/404/409/422/500 as applicable.
- [ ] Run the focused OpenAPI tests and confirm they fail before adding metadata.
- [ ] Add schemas and path registrations using the existing handwritten `ApiDoc` pattern; do not add Swagger UI.
- [ ] Run OpenAPI tests and inspect the served JSON for schema/path mismatches.
- [ ] Commit `docs(api): document order endpoints in OpenAPI`.

### Task 7: Full verification and final review

**Files:**
- Review all changed files; no new production file is expected unless the implementation naturally splits a responsibility.

- [ ] Run `cargo fmt --check`.
- [ ] Run `cargo clippy --all-targets --all-features -- -D warnings`.
- [ ] Run `cargo test --all-targets --all-features`.
- [ ] Run `git diff --check`, `git status`, and `git diff` and inspect for IDOR, pickup-code leakage, predictable pickup codes, N+1 lookups, arithmetic wrapping, mutex-across-await, handler business logic, OpenAPI mismatch, missing negative tests, or unrelated refactors.
- [ ] Fix any issue through a new failing regression test before production changes.
- [ ] Push the current branch only if credentials/workflow permit; never force-push.
- [ ] Commit any final fixes with a focused message and record the final commit hash and exact command results.
