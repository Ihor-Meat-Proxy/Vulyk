# Security policy

## Reporting a vulnerability

Please report security issues **privately**. Do not open a public GitHub issue
for vulnerabilities that could be exploited in production installs.

Preferred channel:

1. **GitHub Security Advisories** — use *Report a vulnerability* on this
   repository (Security → Advisories → New draft advisory / private report).
2. If advisories are unavailable, contact a maintainer privately (for example
   via a direct message to a repo admin) and share a minimal reproduction,
   affected version, and impact assessment.

We aim to acknowledge reports promptly and to coordinate a fix before any
public disclosure.

## Supported versions

Security fixes target the **latest released** version on `main` (and the
current stable tag published to GHCR). Older tags may not receive backports
unless a maintainer explicitly says otherwise. Prefer upgrading with
[`./scripts/install.sh upgrade`](docs/guides/upgrade.md) rather than running
unpatched builds.

## Security posture (operators)

Production-oriented defaults and controls:

- **Auth fail-closed.** Production markers (`VULYK_ENV=production`, HTTPS
  `VULYK_BASE_URL`, or prod Compose/Helm) require authentication. Explicit
  `VULYK_AUTH_REQUIRED=false` in those environments refuses to start. See
  [docs/auth-and-rbac.md](docs/auth-and-rbac.md) and
  [docs/admin-guide.md](docs/admin-guide.md).
- **Secrets.** JWT signing (`VULYK_JWT_SECRET`) and envelope encryption
  (`VULYK_SECRETS_KEY`) must not use known development defaults in production.
  `install.sh` generates secrets into `.env` / `.env.prod` with restrictive
  permissions. API responses mask destination / CI / LLM / SSO secrets.
- **Webhooks & CI.** Provider webhooks reject unsigned traffic when a secret is
  missing (fail-closed). Project CI tokens are scoped for result upload, not
  admin surfaces.
- **Egress.** Outbound URLs (git remotes, backup destinations, ALM, LLM, SSO,
  CI APIs) go through shared SSRF / private-IP guards. Admins may opt in to
  `http` / private CI hosts (Jenkins, Forgejo, GitLab) via Settings or
  `VULYK_CI_ALLOW_PRIVATE_URLS` with an optional allowlist — other egress stays
  strict. Local content-backup paths stay under `VULYK_CONTENT_BACKUP_DIR`.
- **Sessions.** Password change and admin disable/reset invalidate server
  sessions. Cookie sessions use Secure / CSRF controls when TLS or production
  markers are set.
- **Headers & CORS.** API and nginx send security headers; production CORS is
  an allowlist, not `*`.

Operator deploy and env guidance: [docs/admin-guide.md](docs/admin-guide.md),
[docs/guides/first-run-setup.md](docs/guides/first-run-setup.md),
[docs/guides/upgrade.md](docs/guides/upgrade.md).

## Security scanning (developers)

CI runs a dedicated security workflow (cargo audit / deny, npm audit, OSV,
Trivy, CodeQL, Semgrep, gitleaks) on PRs, `main`, and a weekly schedule.
Index and local mirrors:

- [docs/dev/ci-checks.md](docs/dev/ci-checks.md)
- `./scripts/check-security.sh` (and `--quick` for Rust-only)

Justified RustSec ignores live in `deny.toml` and `.cargo/audit.toml` with
review-by dates — prefer dependency upgrades over extending ignores.

## Test-id / DOM attribute rules

Do not put PII, emails, names, passwords, or tokens into `data-testid` or
`data-entity-*` attributes. Convention and enforcement:

- [docs/dev/testing/test-ids.md](docs/dev/testing/test-ids.md)
- `frontend/src/testing/security.ts` (`assertSafeTestAttrs`)

## Further reading

- Auth & RBAC: [docs/auth-and-rbac.md](docs/auth-and-rbac.md)
- Architecture (security summary): [docs/dev/architecture.md](docs/dev/architecture.md)
- Content backups: [docs/guides/backup.md](docs/guides/backup.md)
- Agent / PR guardrails: [AGENTS.md](AGENTS.md)
