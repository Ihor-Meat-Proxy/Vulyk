# Troubleshooting and FAQ

## Session expired

**Symptom:** You are redirected to `/login` with the message that your session expired.

**Causes:**

- JWT or session lifetime ended (`VULYK_JWT_TTL_HOURS` default 12; session cookie TTL default 168 hours)
- Operator rotated `VULYK_JWT_SECRET` (invalidates browser JWTs; a still-valid session cookie may continue until revoked)
- Token cleared after `401` from the API

**What to do:**

1. Sign in again with email/password (or SSO).
2. If a return URL was saved, you land back on the previous page after login.
3. Recreate personal API tokens if scripts still send an old bearer.
4. If you must change password (`must_change_password` / policy), complete the password change — other API routes stay blocked with `403 password_change_required` until then.

Open mode (`VULYK_AUTH_REQUIRED=false`) has no login wall; the gear menu shows an **Open mode** badge. Do not expose open mode on shared or production networks.

## Login rate limits

Login attempts are **rate-limited with exponential backoff**. If sign-in fails repeatedly, wait before retrying. This is separate from GitHub API rate limits used by update checks and automation-repo indexing.

## Update check errors

On **Admin → Updates**, **Check for updates** calls the GitHub Releases API.

| Message pattern | Meaning | What to do |
|-----------------|---------|------------|
| GitHub API **rate limit** exceeded | Too many unauthenticated or quota-exhausted requests | Wait until the shown reset time, then **Retry**. Reducing poll frequency helps |
| **Network** error reaching GitHub | Egress blocked, DNS, or temporary outage | Verify the server can reach GitHub; retry later |
| Releases request **HTTP** error | Non-success response from GitHub | Confirm `VULYK_GITHUB_REPO` and network path; retry |
| Unknown / check failed | Other failure | Retry later; use manual `./scripts/install.sh upgrade` if needed |

One-click **Update now** / rollback require the Compose **updater sidecar** and matching `VULYK_UPDATER_TOKEN`. On Kubernetes or source-only installs, follow the manual commands shown on the page.

## Automation repo rate limits

Indexing or branch refresh against GitHub/GitLab without a token is limited (for example ~60 req/h anonymous on GitHub). Settings shows a translated rate-limit message with **reset in local time** and a hint to add a read-only token. Prefer a token even on public remotes. Polling uses `ETag` so unchanged tips do not burn quota.

## CI ingest problems

| Symptom | Check |
|---------|--------|
| `401` / `403` | CI token revoked, wrong project, or role lacks `upload_results` |
| Empty suite | Results file path; format auto-detect; always-run report step |
| `unmatched_case_keys` | Key typo; case missing; add Playwright annotation / JUnit `test_key` |
| Duplicate runs | Set stable `--external-ref` |
| Rejected after close | Open a new run; closed runs do not accept new results |
| Localhost from GitHub Actions | Tunnel or self-hosted runner; set reachable `VULYK_API_URL` |

Dry-run the Playwright reporter with `VULYK_REPORTER_DRY_RUN=1` while wiring a pipeline.

## Case and search FAQ

**Why can’t I find a short query?**  
Full-text search always runs; contains-style `ILIKE` matching requires at least **3** characters.

**Why was my case key not reused after delete?**  
Retired keys are never reused. Opening them shows **Case deleted**.

**Why can’t I hide the Title column?**  
Title is required in the case list layout.

**PDF export failed or truncated selection?**  
Bulk PDF export allows **at most 100** cases per request. You need **export reports** permission.

## Permissions FAQ

**Viewer can’t execute runs** — expected: Viewer has view/export reports only.  
**Developer can’t create cases** — expected: use Admin / Test Architect / Test Automation Engineer.  
**Chat Create disabled** — your role may allow ask but not confirm-create (see [AI assist](ai.md)).

## Password policy FAQ

Default minimum length is **16** (floor 12) with letter + digit. Existing shorter passwords can still sign in once, then must change. Generated install passwords set **must change** on first login.

## Backup vs upgrade FAQ

- **Admin → Backups** = portable **content** ZIP (projects/cases/fields) to configured destinations.
- **`./scripts/install.sh backup` / upgrade** = verified **Postgres + env** bundle under `./backups/` / `./version-backups/`.
- Never use `docker compose down -v` on a live instance — it deletes named volumes including the database.

## Still stuck?

1. Check `/health` for API liveness and version.
2. Use `/api/docs` to reproduce the failing call with Bearer auth.
3. Review Admin → Updates history / job logs when an in-app upgrade failed (auto-rollback should restore the previous images + DB).
