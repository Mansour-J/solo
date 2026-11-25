# CI/CD

| Workflow | Trigger | Does |
|---|---|---|
| `ci.yml` | Every pull request and push to `main` | The quality gate below, including Conventional Commit messages |
| `release-please.yml` | Every push to `main` | Keeps the "chore: release X.Y.Z" pull request (semantic version, `CHANGELOG.md`); merging it tags and calls `release.yml` |
| `release.yml` | Called by `release-please.yml`, a pushed tag `vX.Y.Z`, or manual | Desktop installers for three OSes and the GitHub release ([release process](release-process.md)) |

## Quality gate (`ci.yml`)

| Job | Checks |
|---|---|
| `commits` | Each pushed commit's message follows Conventional Commits (`scripts/commit-check.mjs`) |
| `api` | `cargo fmt --check`, clippy with warnings as errors, unit and integration tests against a Flyway-migrated PostgreSQL 18 (as `solo_app`, so row-level security is exercised) |
| `database` | Fresh migration, re-run is a no-op, `flyway validate`, upgrade from the previous version, schema integrity checks (row-level security on every business table, cascading foreign keys, append-only audit log, no float columns) |
| `web` | TypeScript, ESLint (zero warnings), Prettier, Vitest, production build, brand configuration applied to the desktop app |
| `contract` | Regenerates the TypeScript client from the API and fails if it differs from the committed one |
| `e2e` | Builds and starts the desktop app's local server (bundled PostgreSQL and the API) and the web client, then runs the Playwright journeys (desktop and phone) |
| `security` | gitleaks secret scan over the history, `npm audit` (production dependencies, high+), `cargo audit` for both crates (reviewed exceptions, with reasons, in `.cargo/audit.toml`) |
| `desktop` | (push only) Linux desktop bundle, Rust tests and a launch smoke test under a virtual display |

Dependabot is switched off for now (no `.github/dependabot.yml`); dependencies are updated by hand, and
the `audit` job still flags vulnerable ones. See SL-001 in the backlog to switch it back on.

## Branch protection (recommended)

Require pull requests into `main` with the `api`, `database`, `web`, `contract`, `e2e` and `security`
checks passing, and disallow force pushes.
