---
name: add-api-endpoint
description: Add or change a Solo API endpoint end to end — pure domain rules, repository SQL, a service with field validation, audit and ledger posting in one business transaction, a thin utoipa handler and route, integration tests including cross-tenant isolation, and the regenerated TypeScript client. Use whenever a feature needs new server data or a new mutation, or an existing request or response shape changes.
---

# Add an API endpoint

Layers: `api` (thin handlers) → `services` (rules, permissions, transactions) → `repositories` (SQL) → `domain` (pure logic). Read `services/api/CLAUDE.md` first.

The income module is the smallest complete example:

- service: `services/api/src/services/income.rs`
- handlers: the income section of `services/api/src/api/expenses.rs`
- SQL: the income section of `services/api/src/repositories/expenses.rs`
- test: `income_without_an_invoice_counts_on_both_bases` in `services/api/tests/bookkeeping.rs`.

If the endpoint needs a new table, follow the `add-migration` skill first. If it changes the books, also follow `ledger-posting`.

## 1. Domain

- Put calculations and rules that need no database in `services/api/src/domain/<module>.rs`, registered in `domain/mod.rs`. No I/O, sqlx, `AppState` or HTTP types.
- Money is `i64` minor units; rates and percentages are `rust_decimal::Decimal`. Round only through `domain::money` (`round_to_minor`, `percentage_of`, `allocate_proportionally` for splits that must sum exactly).
- Unit tests in `#[cfg(test)] mod tests` with worked examples: half-cent rounding, splits, boundary dates.

## 2. Repository

- `services/api/src/repositories/<module>.rs`, registered in `repositories/mod.rs`. Functions take `conn: &mut PgConnection` and return `Result<_, sqlx::Error>`. No rules.
- Runtime-checked queries (`sqlx::query_as` into a `#[derive(sqlx::FromRow)]` struct; ADR 0007). SQLx 0.9 rejects `format!`-built SQL, so define the column list once and build statements with `concat!`:

  ```rust
  macro_rules! thing_columns {
      () => { "t.id, t.name, t.amount, t.created_at, t.updated_at FROM things t" };
  }
  sqlx::query_as(concat!("SELECT ", thing_columns!(), " WHERE t.id = $1"))
  ```

- Row-level security limits every statement to the transaction's business, so `SELECT` and `UPDATE` don't filter on `business_id` (see `find_income`). `INSERT` passes `business_id` explicitly (`insert_income`).
- A row struct returned to clients also derives `serde::Serialize` and `utoipa::ToSchema` (`IncomeRow`).
- `ILIKE` searches escape `\`, `%` and `_` first, as `list_income` does.

## 3. Service

`services/api/src/services/<module>.rs`, registered in `services/mod.rs`:

```rust
pub async fn create(state: &AppState, ctx: &BusinessContext, input: ThingInput) -> AppResult<ThingRow> {
    let mut tx = state.db.begin_business(ctx.user_id(), ctx.business_id).await?;
    let fields = validate(&mut tx, input).await?;
    let id = repo::insert_thing(&mut tx, ctx.business_id, ctx.user_id(), &fields).await?;
    // financial records: sync_postings(&mut tx, …) here (ledger-posting skill)
    audit::record(&mut tx, &ctx.actor(), audit::Entry {
        business_id: Some(ctx.business_id), action: "thing.created", entity_type: "thing", entity_id: Some(id),
        summary: format!("Thing {} created", fields.name), changes: serde_json::json!({ "amount": fields.amount }),
    }).await?;
    let row = repo::find_thing(&mut tx, id).await?.ok_or(AppError::NotFound("Thing"))?;
    tx.commit().await?;
    Ok(row)
}
```

- **No permissions to check** (ADR 0016): there is one owner and no roles. `BusinessContext` (`services/api/src/context.rs`) has already checked the business exists; work inside `begin_business` so row-level security applies.
- **Validation:**
  - Collect errors with `FieldErrors::new()`, `errors.check(ok, "field", "Message.")` and `errors.add(…)`, then `errors.into_result()?`.
  - Field names are the request's JSON field names. Nested ones look like `lines.{index}.{field}` (`domain/documents.rs`).
  - Trim and limit strings with `clean(&value, max, "field", &mut errors)` from `services::invoices`.
  - Messages are UI copy: a full sentence, NZ/UK spelling, typographic apostrophes (`Deleted income can’t be changed.`).
- **References** to other records (customer, category) are looked up inside the transaction. A missing one is a field error (`customer_id`: "Choose a customer."), never a 404: row-level security makes another tenant's ID look missing.
- **Errors:** `AppError::NotFound("Thing")`, `AppError::Conflict("…".into())` for state conflicts, `AppError::Forbidden(…)`. Never put internals in a message.
- **Audit** in the same transaction. Actions are `<entity>.<past tense>`. Updates record `{ "field": { "from": …, "to": … } }` (`income::update`).
- **Financial records** are voided with a reason (`ReasonInput`) instead of deleted, and their postings are reversed through `services::ledger::reconcile`. Never insert or delete journal rows.
- The business's "today" is `business_today(&business.timezone)` from `services::businesses`, not the server clock.

## 4. Handler and route

In `services/api/src/api/<module>.rs`:

```rust
#[utoipa::path(put, path = "/{business_id}/things/{thing_id}", tag = "things",
    params(("business_id" = Uuid, Path), ("thing_id" = Uuid, Path)), request_body = ThingInput,
    responses((status = OK, body = ThingRow), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)), security(("session" = [])))]
async fn update_thing(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<ThingInput>,
) -> AppResult<Json<ThingRow>> {
    Ok(Json(things::update(&state, &ctx, p.id, input).await?))
}
```

- **Paths:** handler arguments can't be tuple patterns (utoipa). Use `Path(p): Path<ItemPath>`, or `Path<SubItemPath>` for a child record, and add the new `thing_id` (and any child `…_id`) as a serde alias in `services/api/src/api/extract.rs`.
- **Extractors:** `ApiJson` and `ApiQuery`, not axum's, so rejections use the standard error body. `BusinessContext` comes from the `{business_id}` segment and returns 404 to non-members.
- **Lists:** `ApiQuery<PageParams>` plus a filter struct deriving `IntoParams` with `#[into_params(parameter_in = Query)]`. Respond with `{ items, page, per_page, total }`: `extract::Page<T>`, or a page struct with totals like `IncomePage`.
- **Status codes:** create returns `(StatusCode::CREATED, Json(…))`; void or delete returns `StatusCode::NO_CONTENT`.
- **Router:** one `.routes(routes!(…))` per path in the module's `router()`. A new module also needs `pub mod` and `.nest("/api/v1/businesses", <module>::router())` in `services/api/src/api/mod.rs`, plus a tag in `ApiDoc`.
- Operation IDs become `<tag>_<handler>`, so keep handler names unique within a tag. JSON conventions: `docs/architecture/api.md`.

## 5. Integration tests

In `services/api/tests/` (`bookkeeping.rs`, or a new file starting with `mod common;`). Helpers in `tests/common/mod.rs`:

- `TestApp::new()`, `app.register(prefix)` (a new owner), `app.create_business(&user, name)`
- `app.get/post/put/delete(path, &user.auth(), …)`
- on the response: `.status`, `.body`, `.field_error("field")`, `.error_code()`.

`Books::new(prefix)` in `bookkeeping.rs` gives a GST-registered business with a customer, plus `assert_ledger_balances()`.

Cover:

- the happy path, including stored totals
- each validation rule: 422 with `field_error("<request field>")`
- state conflicts, such as editing a voided record
- the audit entry: `/api/v1/businesses/{id}/activity` (`updates_are_audited_with_before_and_after_values`)
- for financial records: a balanced ledger and the dashboard figures on both bases
- **cross-tenant** (`bookkeeping_records_are_isolated_between_businesses`):
  - a second user gets 404 on every new path
  - their own business can't reference this business's IDs (field error)
  - a query inside `begin_business(intruder.id, own)` sees no rows
- export: a new table appears in `/api/v1/users/me/export` (`export_and_deletion_cover_bookkeeping_records`).

## 6. Finish

```bash
npm run api:lint
TEST_DATABASE_URL=postgres://solo_app:change-me-app-password@127.0.0.1:55432/solo_test npm run api:test
#   one test: … cargo test --manifest-path services/api/Cargo.toml --test bookkeeping <name>
npm run api:client     # regenerates packages/api-client; commit it (CI's contract job checks)
```

Then use the endpoint from `apps/client/src/features/<module>/api.ts` (`add-feature-page` skill). List the new endpoints in your summary.
