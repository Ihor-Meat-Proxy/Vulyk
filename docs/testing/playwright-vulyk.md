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
- `./scripts/check-e2e-case-keys.sh` **fails** on VQA gaps in
  `playwright-e2e.json`, duplicate `external_key` across seed files, or tags
  that reference missing `VQA`/`VUE` keys.
- **Feature branches:** append the next free `VQA-N` / `VUE-N` by hand. Do
  **not** run sync/generate scripts to renumber. Default sync is additive;
  `--force-renumber` is for owned full regenerations only.

```bash
# After a manual append (preferred on feature branches):
./scripts/check-seed-keys.sh
./scripts/check-e2e-case-keys.sh

# Additive sync (safe: preserves existing keys):
node e2e/scripts/sync-vulyk-case-links.mjs --annotate

# Full rebuild — do NOT use on parallel feature branches:
# node e2e/scripts/sync-vulyk-case-links.mjs --force-renumber --annotate
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
| `VULYK_RUN_ID` | Optional: attach to an existing run (Run in CI / `workflow_dispatch`) instead of opening a new one |
| `VULYK_REPORTER_DRY_RUN` | Optional: `1` / `true` / `yes` — log open/start/results/close **without POSTing** (local DX) |

### Live lifecycle (`onBegin` → stream → `onEnd`)

The reporter is **live**, not end-buffered. Against the CI ingest API:

```mermaid
sequenceDiagram
  participant PW as Playwright
  participant R as vulyk-reporter
  participant API as Vulyk CI API
  PW->>R: onBegin
  alt VULYK_RUN_ID set
    R->>API: POST …/ci/runs/{id}/start
  else new run
    R->>API: POST …/ci/runs (start: true)
  end
  Note over API: Run status = in_progress (visible in Executions)
  loop each finished test
    PW->>R: onTestEnd
    R->>API: POST …/results (batched / timed flush)
  end
  PW->>R: onEnd
  R->>API: flush remaining + POST …/close
  Note over API: closed (or cancelled if interrupted)
```

1. **`onBegin`** — resolve project id, then **open + start** in one step
   (`start: true` on create) so the run is `in_progress` **before** any test
   finishes. If `VULYK_RUN_ID` is set, skip open and call **start** on that run
   (two-way **Run in CI** attach). Failures here disable the reporter for the
   rest of the suite (tests still run).
2. **`onTestEnd`** — map the case key from annotation / tag / title, buffer the
   result, and flush in small batches (or on a short timer) so Executions shows
   progress during a long suite. Retries keep only the last attempt.
3. **`onEnd`** — flush remaining results, then **close**. If Playwright reports
   `interrupted`, close with `cancelled: true` (run status `cancelled`).

A hard-killed runner that never reaches `onEnd` leaves the run visible as
`in_progress`.

### Dry-run

```bash
export VULYK_URL=http://127.0.0.1:8080
export VULYK_CI_TOKEN=vulyk_ci_…   # still required to enable the reporter
export VULYK_PROJECT=VQA
export VULYK_REPORTER_DRY_RUN=1
npx playwright test
```

Stdout shows `[vulyk-reporter] dry-run open|start|results|close: …` lines; no
HTTP traffic is sent. Helper coverage: `npm run test:reporter-helpers` in `e2e/`.

### Unmatched keys and missing annotations

- **Unmatched keys** (case key not in the project): one warning **per key**
  (sample list + total count). Create the case or fix the annotation; unmatched
  keys do **not** fail the Playwright process.
- **Missing annotation**: one warning with a **sample of test titles** + count.
  Annotate every test:

```ts
test('login with seeded admin succeeds', {
  tag: ['@VQA-236', '@VUE-12'],
  annotation: [
    { type: 'vulyk', description: 'VQA-236' },
    { type: 'vulyk', description: 'VUE-12' },
  ],
}, async ({ page }) => {
  // …
})
```

Also accepted: a single `{ type: 'vulyk', description: 'VQA-236' }`, tag
`@VQA-236`, or `VQA-236` in the title. See [CI integration](../guides/ci-integration.md).

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
