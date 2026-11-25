# API conventions

- **Base path:** `/api/v1`. Breaking changes go to `/api/v2`; additive changes don't bump the version.
- **Documentation:** OpenAPI 3.1 at `/api/openapi.json`, interactive reference at `/api/docs`.
  Operation IDs are `<tag>_<handler>` (the generated client relies on them).
- **Resources:** the owner's profile and data export under `/users/me` (there is no sign-in: every request acts as the owner, ADR 0016); everything that belongs to a business under
  `/businesses/{business_id}/…` (customers, invoices, quotes, expenses, income, mileage, allocations, dashboard).
- **JSON:** snake_case fields. Money: integer minor units. Decimals (rates, quantities, percentages):
  strings. Dates: `YYYY-MM-DD`. Timestamps: RFC 3339 UTC.
- **Methods:** `GET` read, `POST` create or action, `PUT` replace (forms send the whole object),
  `DELETE` remove. Deleting a whole business requires a typed confirmation
  in the body.
- **Lists:** `?page=1&per_page=25` (max 100) → `{ items, page, per_page, total }`. Filters and sorting
  are query parameters documented per endpoint.
- **Errors:** `{ "error": { "code", "message", "fields"? } }` with codes `VALIDATION` (422, `fields`
  keyed by request field path), `BAD_REQUEST`, `FORBIDDEN` (a missing header or key), `NOT_FOUND`, `CONFLICT`,
  `PAYLOAD_TOO_LARGE`, `SERVICE_UNAVAILABLE`, `INTERNAL`. Internal errors are logged with their cause
  and returned without it.
- **Files:** PDFs from `…/pdf`; spreadsheet exports from `…/export` as CSV (`text/csv`, UTF-8 with a
  byte-order mark for Excel, formula-like text neutralised), e.g. `/mileage/export?from&to`.
- **Reports:** `/reports/{kind}` returns a report (sections of typed columns and rows, money in minor
  units) and `/reports/{kind}/export?format=pdf|csv|xlsx` the same report as a file, with the shared
  filters `from`, `to`, `basis`, `customer_id`, `category_id`, `vehicle_id` and `status`.
- **Insights:** `GET /insights` calculates insights and notices on request (nothing is stored);
  `POST /insights/dismiss` (`{ key }`) and `POST /insights/restore` manage dismissed notices.
- **Calculations:** forms show figures from the API (`/invoices/calculate`, `/mileage/calculate`), never
  client arithmetic. When editing, the form passes the record (`/mileage/calculate?trip_id=`) so the
  preview accepts what saving would, such as a vehicle archived since.
- **Headers:** state-changing requests must send `x-client-kind: web|desktop|mobile`. The desktop app's local server also requires its per-start `x-api-key` on every `/api` request. Every response
  carries `x-request-id` (also in logs) for tracing.
- **Health:** `/health/live` (process), `/health/ready` (database reachable).
- **Logging:** structured JSON (`LOG_FORMAT=json`) with method, path, status, latency and request ID;
  the `x-api-key` header is marked sensitive and never logged.
