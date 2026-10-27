# Vulyk Playwright e2e

Chromium-only end-to-end suite (Desktop Chrome + Pixel 5 viewport project).

## Locators (required)

**Prefer `data-testid` via shared `TestIds` — avoid ambiguous text locators.**

```ts
import { TestIds } from '../../frontend/src/testing/testIds'

await page.getByTestId(TestIds.loginSubmitButton).click()
// lists: scope by entity attrs, then child test id
await page.locator(`[data-testid="case-row"][data-entity-key="${key}"]`)
  .getByTestId(TestIds.caseRowTitle)
  .click()
```

- Source of truth: [`frontend/src/testing/testIds.ts`](../frontend/src/testing/testIds.ts)
- Convention + security: [`docs/testing/test-ids.md`](../docs/testing/test-ids.md)
- Do **not** use bare `getByText('Save')` / toast text when parallel UI PRs can duplicate those strings (Playwright strict-mode failures).
- When adding UI, add a `TestIds` constant, wire `testIdProps` / `listItemProps`, update the page object here, run `npm run check:test-ids` in `frontend/`.

## Layout

```
e2e/
  fixtures/          # auth/api helpers + sample import files
  pages/             # page objects (getByTestId-first)
  tests/             # focused specs
  mock-oidc/server.mjs
  playwright.config.ts
```

## Prerequisites

- API on `VULYK_API_URL` (default `http://127.0.0.1:8080`)
- UI on `E2E_BASE_URL` (default `http://127.0.0.1:5173`, Vite preview with `/api` proxy)
- Seeded admin via env (recommended):

```bash
export VULYK_AUTH_REQUIRED=true
export VULYK_ADMIN_EMAIL=admin@vulyk.local
export VULYK_ADMIN_PASSWORD=admin12345678901
export VULYK_SEED_DEMO=1
export VULYK_DEMO_EMAIL=user@vulyk.local
export VULYK_DEMO_PASSWORD=user123456789012
export VULYK_REGISTRATION_MODE=open
export VULYK_LLM_MOCK=1
export VULYK_DEFAULT_LOCALE=en
export VULYK_DEFAULT_TIMEZONE=UTC
export VULYK_BASE_URL=http://127.0.0.1:5173   # required for SSO callback via UI proxy
export DATABASE_URL=postgres://vulyk:vulyk@localhost:5432/vulyk
```

## Run

```bash
cd e2e
npm ci
npx playwright install --with-deps chromium
npm run typecheck
npm test
```

Useful filters:

```bash
npm run test:chromium
npm run test:mobile
npx playwright test tests/auth.spec.ts
```

## Mock OIDC (SSO)

`global-setup.ts` starts `mock-oidc/server.mjs` on port **9090** (override with `E2E_OIDC_PORT`).
The mock issues **RS256-signed** `id_token`s and serves a matching JWKS so the API can verify signatures (audit H7).

The SSO spec enables the seeded `oidc` provider via admin API:

- `issuer` = `http://127.0.0.1:9090`
- `client_id` / `client_secret` = `vulyk-e2e` / `vulyk-e2e-secret`

The API process needs `VULYK_EGRESS_ALLOW_PRIVATE=1` so JWKS fetch to the local mock is allowed (CI sets this; never enable in production).

Set `E2E_SKIP_MOCK_OIDC=1` to skip starting the mock (SSO test will skip).

## Setup wizard

Uses `DATABASE_URL` + `psql` to flip `setup_completed=false`, insert a known setup token hash, exercise the wizard (bad token → 403/error, good token + locale `uk` + timezone), then restores setup complete in `afterAll`.

Also asserts `cargo run -p vulyk-api -- setup --help`.
