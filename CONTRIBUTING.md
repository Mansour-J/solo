# Contributing

## Workflow

- Branch from `main`, open a pull request and complete the template. CI must be green to merge.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/):
  `<type>[(scope)][!]: <description>`, e.g. `feat(invoices): email a statement to the customer`. The
  types: `feat` (a new feature: minor version), `fix`, `perf` and `deps` (patch), `revert`, and `docs`,
  `refactor`, `test`, `build`, `ci`, `style`, `chore` (no release on their own, not in the changelog). A `!`
  after the type or a `BREAKING CHANGE:` footer marks a breaking change (major; minor while the version
  is 0.x). The description is what users read in the changelog, so write it for them. `npm install` sets
  up a `commit-msg` hook that checks this (`scripts/commit-check.mjs`), and CI checks every pushed commit.
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

Releases follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html) and are made on GitHub by
release-please: it keeps a release pull request ("chore: release X.Y.Z") with the next version and the new
`CHANGELOG.md` section, and merging it tags, builds and publishes the release.

See [docs/deployment/release-process.md](docs/deployment/release-process.md).
