# Catalog V1 — read-only HTTP contract

The catalog slice exposes products and categories for reading. This is the
contract the frontend builds against. It is described formally in the OpenAPI
document served at `GET /api/openapi.json`.

All endpoints in this slice are unauthenticated and only support `GET`.
Authentication lives in `docs/api/authentication.md`.

## Money representation

Prices are always integers in **minor units (centavos)**. The API never
serializes decimal strings: `500` means R$ 5,00, `750` means R$ 7,50, `1250`
means R$ 12,50. Formatting into a decimal representation is a presentation
concern handled by the client (see `cantina-cli`'s `R$ 5,00` human output).

## Endpoints

### `GET /api/v1/products`

Lists products. By default only products that are currently **available** are
returned.

Query parameters (all optional):

| Parameter | Type | Default | Meaning |
| --- | --- | --- | --- |
| `category_id` | string | — | Restricts the results to one category. |
| `search` | string | — | Case-insensitive substring match against the name or description. |
| `include_unavailable` | boolean | `false` | When `true`, unavailable products are included. |

Results are ordered deterministically by name, then by identifier.

Response `200`:

```json
{
  "items": [
    {
      "id": "product_brigadeiro",
      "name": "Brigadeiro",
      "description": "Chocolate truffle, school classic",
      "price": 500,
      "category_id": "category_snacks",
      "available": true
    }
  ]
}
```

Invalid query values (for example `include_unavailable=not-a-bool`) return a
`400` with the standardized `INVALID_QUERY` error body. The message is a
stable `"the catalog query is invalid"`; framework or parser internals are
never included, and the request identifier is attached as usual.

```bash
curl 'http://127.0.0.1:8080/api/v1/products'
curl 'http://127.0.0.1:8080/api/v1/products?category_id=category_snacks'
curl 'http://127.0.0.1:8080/api/v1/products?search=coxinha&include_unavailable=true'
```

The administrative CLI maps this contract directly:

```bash
cargo run -p cantina-cli -- products list                 # available only
cargo run -p cantina-cli -- products list --all           # include unavailable
cargo run -p cantina-cli -- products list --include-unavailable  # alias of --all
```

`--all` is the canonical flag; `--include-unavailable` is accepted as an
alias and produces the exact same request.

### `GET /api/v1/products/{product_id}`

Returns a single product. `404` with a `NOT_FOUND` error body when the product
does not exist.

Response `200`:

```json
{
  "id": "product_brigadeiro",
  "name": "Brigadeiro",
  "description": "Chocolate truffle, school classic",
  "price": 500,
  "category_id": "category_snacks",
  "available": true
}
```

### `GET /api/v1/categories`

Lists all categories, ordered deterministically by name, then by identifier.

Response `200`:

```json
{
  "items": [
    { "id": "category_snacks", "name": "Snacks" }
  ]
}
```

### `GET /api/v1/categories/{category_id}`

Returns a single category. `404` with a `NOT_FOUND` error body when the
category does not exist.

### `GET /api/openapi.json`

Returns the machine-readable OpenAPI 3.1 document describing this contract.
No interactive UI is served; the document exists for tooling and generated
clients.

## Errors

Every non-success response uses a single envelope:

```json
{
  "error": {
    "code": "NOT_FOUND",
    "message": "the product with id 'product_missing' was not found",
    "request_id": "3b3..."
  }
}
```

`request_id` is attached by the request-id middleware and is omitted when the
middleware is not present.

| Code | HTTP status | Meaning |
| --- | --- | --- |
| `INVALID_QUERY` | `400` | The query string could not be parsed. |
| `NOT_FOUND` | `404` | The requested resource does not exist. |
| `INTERNAL_ERROR` | `500` | An internal storage failure occurred. |

## Scope

This slice is strictly read-only. Mutations, authentication, pagination and
stock tracking are deliberately out of scope for now; the OpenAPI document
describes only what exists today, not future intent.
