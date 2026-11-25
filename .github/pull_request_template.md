## What and why

<!-- What changes, which roadmap item, and why. -->

## Checklist

- [ ] `npm run check` and `npm run api:test` pass (CI also runs e2e, migrations and security checks)
- [ ] API changed? `npm run api:client` re-run and the generated client committed
- [ ] Database changed? A **new** Flyway migration (never an edited one); business tables call `enable_tenant_isolation()`
- [ ] Money is integer minor units; calculations live in `services/api/src/domain` with tests
- [ ] Tenant isolation covered by a test for new business-scoped endpoints
- [ ] No hardcoded product name, colours or URLs (use the brand configuration)
- [ ] No dead UI: every control works, or is disabled with a reason
- [ ] Checked in light mode, dark mode and on a phone-sized screen (screenshots below)
- [ ] Docs updated (architecture, roadmap status, environment variables)

## Screenshots
