# Contributing

## Workflow

- Branch from `main`, open a pull request and complete the template. CI must be green to merge.
- Commit subjects in the imperative ("Add invoice PDFs"): they become the release notes.
- Record significant decisions as an ADR in `docs/decisions/`.
- Update the roadmap status in `docs/product/roadmap.md` when a feature is done.

## Definition of done

A feature is done when its UI, API and migrations exist; validation works on both sides; business
logic is correct and lives in the domain/service layers; errors and empty/loading states are handled;
tenant isolation is enforced and tested; tests are meaningful (unit for rules,
integration against PostgreSQL for endpoints, Playwright for journeys); and docs are updated.

## Checks

```bash
npm run check && npm run api:lint && npm run api:test
npm run api:client          # if the API changed; commit the generated files
npm run test:e2e            # against the running stack
```

## Rules that matter most

1. Money is integer minor units; calculations live in `services/api/src/domain` with tests.
2. Business data is accessed only through `BusinessContext` and `begin_business` transactions.
3. Database changes are new Flyway migrations; business tables enable tenant isolation.
4. No hardcoded branding; no dead UI; never show invented financial figures.
5. Never put secrets in client code or `VITE_*` variables.

## Releasing

```bash
npm run release:version -- 0.2.0
git commit -am "Release v0.2.0" && git tag -a v0.2.0 -m "Release v0.2.0" && git push origin HEAD v0.2.0
```

See [docs/deployment/release-process.md](docs/deployment/release-process.md).
