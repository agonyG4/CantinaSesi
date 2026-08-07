# Catalog Administration V1 — mutation HTTP contract

This slice adds authenticated product and category mutations on top of the
read-only [Catalog V1](./catalog.md) contract. All reads remain unauthenticated
and unchanged.

Mutation endpoints require `Authorization: Bearer <access-token>` and enforce
role policy in the application layer, not only at the HTTP boundary.

## Money representation

Prices are always integers in **minor units (centavos)**. `1200` means
R$ 12,00. Clients never send or receive decimal strings, and the CLI never
parses prices with floating point.

## Role matrix

| Operation               | Student | Canteen | Admin |
| ----------------------- | :-----: | :-----: | :---: |
| Read products/categories | allowed | allowed | allowed |
| Create products         | forbidden | allowed | allowed |
| Update products         | forbidden | allowed | allowed |
| Enable/disable products | forbidden | allowed | allowed |
| Create categories       | forbidden | forbidden | allowed |
| Update categories       | forbidden | forbidden | allowed |

Authorization is enforced by the application use cases (`AuthorizationService`).
HTTP handlers only authenticate the request and adapt DTOs; they never re-check
roles independently.

## Product endpoints

### `POST /api/v1/products`

Creates a product. Requires **Canteen or Admin**. The product identifier is
generated server-side and is never accepted from the client.

Request:

```json
{
  "name": "Sanduíche natural",
  "description": "Pão integral, frango e salada",
  "price": 1200,
  "category_id": "category_snacks",
  "available": true
}
```

`available` defaults to `true` when omitted.

Response `201 Created`, with a `Location` header of
`/api/v1/products/{generated-id}`:

```json
{
  "id": "product_...",
  "name": "Sanduíche natural",
  "description": "Pão integral, frango e salada",
  "price": 1200,
  "category_id": "category_snacks",
  "available": true
}
```

A `404` is returned when the referenced `category_id` does not exist.

### `PATCH /api/v1/products/{product_id}`

Updates a product. Requires **Canteen or Admin**. All request fields are
optional and only provided fields change; at least one must be present (an
empty update is `422 EMPTY_UPDATE`). Availability cannot be changed through
this endpoint.

```json
{
  "name": "Sanduíche natural especial",
  "price": 1350
}
```

An explicit empty string `description` clears the description.

Response `200 OK` with the updated product. `404` when the product or a newly
referenced `category_id` does not exist.

### `PATCH /api/v1/products/{product_id}/availability`

Enables or disables a product. Requires **Canteen or Admin**. Disabling is how
a product is taken off the public menu (products are never deleted).

```json
{ "available": false }
```

The operation is idempotent: enabling an already-available or disabling an
already-unavailable product succeeds. Response `200 OK` with the updated
product. `404` when the product does not exist.

## Category endpoints

### `POST /api/v1/categories`

Creates a category. Requires **Admin** only. The identifier is generated
server-side.

```json
{ "name": "Combos" }
```

Response `201 Created` with a `Location` header of
`/api/v1/categories/{generated-id}` and the created category.

Category names are unique case-insensitively and whitespace-insensitively
(`Snacks`, `snacks` and `  SNACKS ` are equal). A duplicate returns
`409 CATEGORY_NAME_CONFLICT`.

### `PATCH /api/v1/categories/{category_id}`

Renames a category. Requires **Admin** only.

```json
{ "name": "Lanches e combos" }
```

Response `200 OK` with the updated category. A name equal to the category's own
current name succeeds; a name equal to another category's (case-insensitively)
returns `409 CATEGORY_NAME_CONFLICT`. `404` when the category does not exist.

## Error mapping

Every non-success response uses the standardized envelope documented in
`catalog.md`, always carrying the current `request_id`.

| Code | HTTP status | Meaning |
| --- | --- | --- |
| `INVALID_REQUEST_BODY` | `400` | Malformed or oversized body. |
| `AUTHENTICATION_REQUIRED` / `INVALID_ACCESS_TOKEN` / `SESSION_EXPIRED` | `401` | No valid bearer token. |
| `FORBIDDEN` / `ACCOUNT_INACTIVE` | `403` | Role not permitted or disabled account. |
| `NOT_FOUND` | `404` | Product or category does not exist. |
| `CATEGORY_NAME_CONFLICT` | `409` | Category name already in use. |
| `INVALID_PRODUCT_NAME`, `PRODUCT_NAME_TOO_LONG`, `PRODUCT_DESCRIPTION_TOO_LONG`, `INVALID_PRODUCT_PRICE`, `INVALID_CATEGORY_NAME`, `CATEGORY_NAME_TOO_LONG`, `EMPTY_UPDATE` | `422` | Domain validation failure. |
| `INTERNAL_ERROR` | `500` | Internal/repository failure. |

Generated identifier collisions are treated as internal `500`s, never as client
conflicts.

## CLI commands

The CLI talks only to this HTTP API. It surfaces server-side `401`/`403`/`409`
errors verbatim and never assumes role rules.

```bash
cantina --token <token> products create \
  --name "Sanduíche natural" \
  --description "Pão integral, frango e salada" \
  --price-cents 1200 \
  --category-id category_snacks
cantina --token <token> products update product_123 --name "Novo nome" --price-cents 1300
cantina --token <token> products enable product_123
cantina --token <token> products disable product_123

cantina --token <token> categories create --name "Combos"
cantina --token <token> categories update category_123 --name "Lanches e combos"
```

Human mode prints the returned product/category using the same formatting as
`products list`/`show`; `--format json` prints the exact API response.

## Persistence

Storage is in-memory only. Mutations apply to the running process and data
disappears when the API restarts. See ADR 0003 for the intended migration path.
