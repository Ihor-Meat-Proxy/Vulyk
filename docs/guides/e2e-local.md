# Local Playwright e2e (Docker)

Run the full UI suite against an **isolated** Vulyk stack (own Compose project,
Postgres volume, and ports) using the official Playwright image pinned to the
same version as `@playwright/test` in `e2e/package-lock.json`.

This path is for **local** runs. It is not a required CI gate. Required CI still
uses the existing `playwright-e2e` job in `.github/workflows/ci.yml`.

## One-liner

```bash
./scripts/e2e-local.sh
```

First run builds `vulyk-api:e2e` / `vulyk-web:e2e` (slow once), starts
`db` + `api` + `web` + `mock-oidc`, then runs Playwright in
`mcr.microsoft.com/playwright:v<lockfile>-jammy`.

HTML report: `e2e/artifacts/html-report/latest/index.html`  
Traces / failures: `e2e/artifacts/test-results/`

## What it isolates

| Concern | Behaviour |
| --- | --- |
| Operator `.env` | Untouched. Uses `e2e/.docker.env` (from `e2e/docker.env.example`). |
| Default DB volumes | Untouched. Volume is `vulyk_e2e_pg` under Compose project `vulyk-e2e`. |
| Host ports | Defaults `15432` (db), `18080` (api), `15173` (web) — override in `e2e/.docker.env`. |

## Common commands

```bash
./scripts/e2e-local.sh --build          # rebuild api/web images
./scripts/e2e-local.sh --sync           # upsert VUE project + cases before run
./scripts/e2e-local.sh --report         # sync + mint CI token + post results into VUE
./scripts/e2e-local.sh --repeat 3       # flake gate (must be green 3×; wipe once at start)
./scripts/e2e-local.sh --keep-data      # reuse e2e DB volume (default wipes volumes first)
./scripts/e2e-local.sh --ui             # Playwright UI mode
./scripts/e2e-local.sh --video          # record video for every test (PW_VIDEO=on)
./scripts/e2e-local.sh --video=retain-on-failure
./scripts/e2e-local.sh --down           # stop stack (keep volume)
./scripts/e2e-local.sh --reset          # stop stack and delete e2e volume (exit)
./scripts/e2e-local.sh -- tests/auth.spec.ts   # pass-through Playwright args
```

By default each invocation **wipes the e2e Compose volumes** before `up`, so
leftover projects / automation repos / backups from earlier runs cannot poison
locators. Pass `--keep-data` to skip the wipe. `--reset` still means wipe-and-exit
without running Playwright.

Compose file: [`docker-compose.e2e.yml`](../../docker-compose.e2e.yml).

## Debug

- **HTML report:** open `e2e/artifacts/html-report/latest/index.html`.
- **Trace:** failed tests keep a trace under `e2e/artifacts/test-results/` — open with
  `npx playwright show-trace <path>` (from `e2e/` on the host, or inside the image).
- **Video (opt-in):** default is off. Pass `--video` / `--video=on` /
  `--video=retain-on-failure` / `--video=off`, or set `PW_VIDEO` the same way.
  The script exports `PW_VIDEO` into the Playwright container; `e2e/playwright.config.ts`
  maps it to Playwright's `use.video`. Files land under `e2e/artifacts/test-results/`.
- **UI mode:** `./scripts/e2e-local.sh --ui` then browse to the printed UI port.
- **API logs:** `docker compose -f docker-compose.e2e.yml --env-file e2e/.docker.env logs -f api`.
- **Skip mock OIDC in container:** already set (`E2E_SKIP_MOCK_OIDC=1`); issuer is
  `http://mock-oidc:9090` on the Compose network.

## Vulyk UI E2E project (`VUE`)

Committed source of truth: [`e2e/cases/`](../../e2e/cases/) (+ `manifest.json`).

```bash
# Regenerate cases from specs (and rewrite @VUE-N tags)
node e2e/scripts/generate-vue-cases.mjs --annotate

# Upsert project/suites/cases via API (idempotent by external_key)
VULYK_API_URL=http://127.0.0.1:18080 node e2e/scripts/sync-vue-project.mjs --annotate --mint-ci-token
```

Each Playwright test carries both:

- `@VQA-N` — sample **Vulyk QA** seed / dogfood (existing reporter + seed fragment)
- `@VUE-N` — dedicated **Vulyk UI E2E** project

The reporter posts the key matching `VULYK_CASE_KEY_PREFIX` (default `VUE` in the
local script). See also [playwright-vulyk.md](../testing/playwright-vulyk.md) and
[ci-integration.md](./ci-integration.md).

## Add a test + case

1. Add/extend a spec under `e2e/tests/` using page objects + `TestIds`.
2. **On feature branches:** append the next free `VQA-N` to
   `backend/seed/vulyk-qa.d/playwright-e2e.json` and the next free `VUE-N` to
   `e2e/cases/<folder>.json` + `manifest.json` by hand. Prefer a narrative
   fragment with **keys omitted** for non-e2e sample cases. Then run
   `./scripts/check-seed-keys.sh` and `./scripts/check-e2e-case-keys.sh`.
   Do **not** run `sync-vulyk-case-links.mjs` / `generate-vue-cases.mjs` to
   renumber (additive sync is OK; `--force-renumber` is not for parallel PRs).
3. Optional owned regen only: `node e2e/scripts/sync-vulyk-case-links.mjs --annotate`
   (additive) or `--force-renumber` when you intentionally rebuild inventories.
4. Optionally `./scripts/e2e-local.sh --sync` against a running stack.
5. Cover positive, negative, and edge paths for the feature.

## Coverage matrix

Snapshot (103 cases / 100 runnable tests + 3 skipped + meta audit; project key **VUE**):

| Feature area | Tests | Positive | Negative | Edge | Case keys |
| --- | ---: | ---: | ---: | ---: | --- |
| Auth | 12 | 10 | 2 | 0 | VUE-1… |
| Projects & cases | 16 | 15 | 0 | 1 | |
| Folder tree / DnD | (in Projects & Edge) | | | | |
| Settings | 13 | 12 | 0 | 1 | |
| Admin | 9 | 9 | 0 | 0 | |
| Automation & CI | 8 | 8 | 0 | 0 | |
| Imports & reports | 8 | 8 | 0 | 0 | |
| Edge cases | 7 | 2 | 2 | 3 | |
| LLM & chat | 6 | 6 | 0 | 0 | |
| Sample data | 5 | 5 | 0 | 0 | |
| Executions | 5 | 5 | 0 | 0 | |
| RBAC | 5 | 3 | 2 | 0 | |
| PWA & a11y | 3 | 2 | 0 | 1 | |
| Requirements | 2 | 2 | 0 | 0 | |
| Plans & runs | 1 | 0 | 1 | 0 | |
| Shell / Smoke / Meta | 3 | 3 | 0 | 0 | |
| **Total** | **103** | **90** | **7** | **6** | VUE-1…VUE-103 |

Regenerate counts anytime with:

```bash
node -e "
const m=require('./e2e/cases/manifest.json');
const fs=require('fs');
let pos=0,neg=0,edge=0;
for (const f of m.folders) {
  const d=JSON.parse(fs.readFileSync('e2e/cases/'+f.file,'utf8'));
  for (const c of d.cases) {
    if (c.type==='negative') neg++;
    else if (c.type==='edge') edge++;
    else pos++;
  }
  console.log(f.name.padEnd(22), String(f.count).padStart(3), f.file);
}
console.log('TOTAL', m.case_count, {positive:pos, negative:neg, edge});
"
```

The committed matrix snapshot lives in [`e2e/cases/manifest.json`](../../e2e/cases/manifest.json)
(`folders[].count`, `case_count`). Feature PRs must keep it in sync (see `AGENTS.md`).

## Upgrade path

`docker-compose.e2e.yml` and `./scripts/e2e-local.sh` are **additive** local-only
tooling. They do not change operator `.env`, default Compose services, volumes, or
`install.sh upgrade`. No migration or env sync markers are required for this guide.
