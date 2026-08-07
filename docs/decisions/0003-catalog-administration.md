# ADR 0003: Catalog administration — products are disabled, not deleted

- Status: accepted
- Date: 2026-08-07

## Context

Catalog V1 (ADR 0001) shipped a read-only catalog. The next backend stage adds
administrative product and category mutations. The design had to settle how
products become unavailable, whether physical deletion exists, and who is
allowed to change the catalog.

## Decision

### Products are disabled, not deleted

A product leaves the public menu by setting `available = false`, never by
physical deletion. No `DELETE /products/{id}` endpoint exists in this stage.
Disabling preserves order history references and audit context; re-enabling is
a simple `PATCH .../availability`. A future database adapter must therefore
model availability as a column update rather than a row removal.

### Categories are not deletable in V1

No category deletion endpoint is introduced. Products already reference
categories by identifier; deleting a category would either orphan products or
require cascade rules that are out of scope for V1. Category taxonomy is
edited by renaming, not removal.

### Canteen and Admin manage products

Products (create, update, enable/disable) are allowed for the **Canteen** role
(day-to-day menu operator) and **Admin**.

### Admin manages category taxonomy

Category creation and rename are **Admin-only**. Category names are unique
case-insensitively and whitespace-insensitively, so duplicate taxonomy is
prevented at the repository boundary.

### Application use cases enforce RBAC

Authorization lives in the application layer, not in HTTP handlers. Handlers
only authenticate the request and adapt DTOs; the use cases are the final
authorization authority. This keeps the policy enforceable regardless of the
transport.

## Consequences

- Products are soft-disabled; the read API already filters unavailable products
  by default.
- No physical deletion contract exists, keeping order references intact.
- Role policy is centralized and testable at the application boundary.
- In-memory persistence means mutations are volatile; a database adapter must
  preserve create/update/conflict semantics (no silent upsert, identifier
  conflicts, case-insensitive category-name uniqueness).
