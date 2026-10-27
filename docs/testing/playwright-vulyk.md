# Link Playwright e2e to Vulyk cases

Dogfood Vulyk by mapping every `e2e/tests/*.spec.ts` test to a Vulyk QA case
(`VQA-N`), then optionally streaming results into a local (or remote) Vulyk
project with the built-in Playwright reporter.

## Case keys on every test

Each test declares its case key via Playwright **tag** + **annotation**:

```ts
test('login with seeded admin succeeds', {
  tag: '@VQA-236',
  annotation: { type: 'vulyk', description: 'VQA-236' },
}, async ({ page }) => {
  // …
})
```

- Seed cases live in `backend/seed/vulyk-qa.d/playwright-e2e.json` (registered in
  `VULYK_QA_FRAGMENTS`). Each case’s `external_key` is
  `e2e:{spec file} {exact test title}` (automation link).
- `./scripts/check.sh` (and `npm run check:vulyk-case-links` in `e2e/`) **fails**
  when a spec test is missing the annotation.
- To refresh after adding a test:

```bash
node e2e/scripts/sync-vulyk-case-links.mjs --annotate
./scripts/check-seed-keys.sh
```

## Reporter (optional)

`e2e/reporters/vulyk-reporter.ts` is always registered in
`playwright.config.ts`. It is a **silent no-op** unless all of these are set:

| Env | Purpose |
| --- | --- |
| `VULYK_URL` | API base (`http://127.0.0.1:8080`). Alias: `VULYK_API_URL` |
| `VULYK_CI_TOKEN` | Project CI token (`vulyk_ci_…`). Alias: `VULYK_API_TOKEN` |
| `VULYK_PROJECT` | Project **key** (e.g. `VQA`) or UUID. Aliases: `VULYK_PROJECT_KEY`, `VULYK_PROJECT_ID` |
| `VULYK_RUN_NAME` | Optional suite display name (default `Playwright e2e`) |

Lifecycle against the CI ingest API: **open → start (in_progress) → POST results
by `case_key` → close**. Unmatched keys are logged; they do not fail the
Playwright process.

## Run e2e against a local Vulyk and see real results

1. Start API + UI with sample seed (includes the Playwright e2e suite under
   Vulyk QA):

```bash
export DATABASE_URL=postgres://vulyk:vulyk@localhost:5432/vulyk
export VULYK_AUTH_REQUIRED=true
export VULYK_ADMIN_EMAIL=admin@vulyk.local
export VULYK_ADMIN_PASSWORD=admin12345678901
export VULYK_SEED_SAMPLE=1
# …other e2e env from e2e/README.md
cargo run -p vulyk-api
# and frontend preview on :5173
```

2. Sign in as admin → open the **Vulyk QA** project → **Settings → Project CI
   tokens** → create a token → copy `vulyk_ci_…` once.

3. Run Playwright with reporting enabled:

```bash
cd e2e
export VULYK_URL=http://127.0.0.1:8080
export VULYK_CI_TOKEN=vulyk_ci_…          # paste secret
export VULYK_PROJECT=VQA                  # or your project key
export VULYK_RUN_NAME="Local Playwright $(date +%H:%M)"
npx playwright test
```

4. In Vulyk, open **Executions** (or the run linked from the CI suite name) to
   see pass/fail by case key.

Omit the three env vars for a normal CI/local run with no Vulyk traffic.

## External consumers

Copy `e2e/reporters/vulyk-reporter.ts` into your own Playwright repo (or depend
on this tree), register it in `playwright.config`, annotate tests with
`{ type: 'vulyk', description: 'YOUR-KEY' }`, and set the same env vars. See
also [CI integration](../guides/ci-integration.md) for JUnit / `vulyk-upload.sh`
pipelines.
