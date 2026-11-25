# 0003 — Tenant isolation

- **Status:** Accepted
- **Date:** 2026-11-20

## Context

One user can own several businesses; each business's financial data must be invisible to anyone who
isn't a member. A missing `WHERE business_id = …` is the classic multi-tenant bug.

## Decision

- The **business** is the tenant. Business data lives under `/api/v1/businesses/{business_id}/…`;
  the `BusinessContext` extractor verifies membership and returns 404 to non-members.
- **Row-level security** on every business table (`enable_tenant_isolation()`), comparing
  `business_id` with the transaction setting `solo.business_id`. The API connects as `solo_app`, which
  owns nothing and so cannot bypass policies. Business data is only read inside
  `Db::begin_business(user, business)` transactions.
- CI checks every `business_id` table has a policy and every foreign key to `businesses` cascades;
  integration tests prove cross-tenant reads and writes fail even when issued directly in SQL.

## Consequences

- Business requests run in a transaction (also giving atomic writes with their audit entries).
- New tables must call `enable_tenant_isolation()` and grant privileges explicitly; CI catches omissions.
- Cross-business administrative queries need the owner role (backups, maintenance), not the API.

## Alternatives considered

- **Application checks only:** one forgotten filter exposes data.
- **Database per tenant / schema per tenant:** heavy operations and migrations for many small businesses.
- **Tenant in a header instead of the path:** less explicit in logs and harder to audit.
