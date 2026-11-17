# Supply-chain policy

How Vulyk keeps dependencies as fresh as possible without blindly trusting
brand-new or hijacked releases. Implemented for
[#294](https://github.com/Ihor-Meat-Proxy/Vulyk/issues/294).

## Checklist (#294)

| Control | Where |
| --- | --- |
| Dependabot cooldown (3d patch / 7d minor+major) | [`.github/dependabot.yml`](../../.github/dependabot.yml) |
| Security fast path (skip cooldown; auto-merge when green) | Dependabot + [`dependabot-auto-merge.yml`](../../.github/workflows/dependabot-auto-merge.yml) |
| Locked installs (`cargo --locked`, `npm ci`) | CI, Dockerfiles, [`npm-ci-secure.sh`](../../scripts/npm-ci-secure.sh) |
| Block npm lifecycle scripts (+ allowlist) | `frontend/.npmrc` / `e2e/.npmrc`, allowlist file |
| `min-release-age=7` with security override | `.npmrc` + `--min-release-age=0` (below) |
| Dep-PR scans (deny/audit, OSV on changed pkgs, npm heuristics) | [`dependabot-dep-scan.yml`](../../.github/workflows/dependabot-dep-scan.yml) |
| New package / maintainer / install-script review gate | [`dependabot-review-gate.yml`](../../.github/workflows/dependabot-review-gate.yml) |
| CI isolation (read-only token, no secrets on dep PRs) | Review gate + dep scan `permissions:` (below) |
| Signed releases (cosign + provenance + SPDX SBOM) | [`release.yml`](../../.github/workflows/release.yml) |
| Optional verify on upgrade | `install.sh --verify-images` / `VULYK_VERIFY_IMAGES` |
| Dev Container (toolchain isolated from host) | [`.devcontainer/`](../../.devcontainer/) |

## Dependabot

- **Cooldown** (version updates only): 3 days for patches, 7 days for minors and
  majors — configured in [`.github/dependabot.yml`](../../.github/dependabot.yml).
  **Security updates skip cooldown** (Dependabot platform behaviour).
- **Auto-merge**: patch and ≥1.x minor once CI is green; **security-advisory**
  PRs also auto-merge (including majors). New packages, new maintainers, or new
  npm install scripts block auto-merge until owner review
  (`supply-chain-reviewed`). See [ci-checks.md](ci-checks.md).

## Locked installs

- Rust: `cargo … --locked` in CI, Docker, and helper scripts.
- npm: `./scripts/npm-ci-secure.sh` runs `npm ci --ignore-scripts` (fails if the
  lockfile is out of sync). Lifecycle scripts are blocked by
  `frontend/.npmrc` / `e2e/.npmrc` (`ignore-scripts=true`) unless a package is
  listed in [`scripts/npm-install-scripts-allowlist.txt`](../../scripts/npm-install-scripts-allowlist.txt)
  and rebuilt after install.

## npm `min-release-age` and the security fast path

Project `.npmrc` sets:

```ini
min-release-age=7
```

so routine `npm install` / resolve flows prefer versions at least seven days
old (complements Dependabot cooldown). That must **not** block installing a
security fix published less than seven days ago when Dependabot opens an
advisory PR.

### Override (per install)

CLI config beats project `.npmrc`. The install helper adds
`--min-release-age=0` when the security fast path is active:

```bash
# Explicit (local / one-off)
VULYK_NPM_SECURITY_FAST_PATH=1 ./scripts/npm-ci-secure.sh frontend
# or
./scripts/npm-ci-secure.sh frontend --security-fast-path
```

[`scripts/npm-security-fast-path.sh`](../../scripts/npm-security-fast-path.sh)
prints `1` when:

1. `VULYK_NPM_SECURITY_FAST_PATH` is explicitly `1` / `true`, or
2. `GITHUB_ACTOR` is `dependabot[bot]` and the pull_request event has a
   `security` label or advisory markers (`GHSA-…`, `CVE-…`, “security
   vulnerability”, …).

CI `npm ci` jobs call `npm-ci-secure.sh`, so Dependabot security PRs pick this
up automatically from `GITHUB_EVENT_PATH`. Docker web builds pass

```text
--build-arg VULYK_NPM_SECURITY_FAST_PATH=<0|1>
```

into [`frontend/Dockerfile`](../../frontend/Dockerfile) (resolved by the same
helper in the Docker / Security workflows).

### Proof

```bash
./scripts/check-npm-security-fast-path.sh
```

Asserts `.npmrc` age=7, that `--min-release-age=0` overrides it, that Dependabot
security event detection returns `1`, and that `npm-ci-secure.sh` only passes
`--min-release-age=0` when the fast path is on. Also runs in
`./scripts/check.sh` and the **npm security fast path** Fast CI gate.

## Dependency PR scanning

Every PR already runs cargo-deny (Fast CI), cargo-audit + OSV + npm audit
(Security). Dependabot PRs get an extra lane
[`.github/workflows/dependabot-dep-scan.yml`](../../.github/workflows/dependabot-dep-scan.yml):

| Check | Role |
| --- | --- |
| cargo deny + cargo audit | Hard fail on advisory / license / source policy (honors `deny.toml` / `.cargo/audit.toml`) |
| OSV on changed packages | `scripts/check-osv-changed-deps.sh` fails only if a package whose version changed in the PR has an OSV hit (baseline noise stays in Security SARIF) |
| npm lock heuristics | `scripts/check-npm-lock-heuristics.py` - new install scripts, registry host flips, typosquat-ish new names |

The Dependabot review gate also runs the npm heuristics alongside the new-package /
maintainer / install-script gate. Override with label `supply-chain-reviewed`.

```bash
BASE_REF=origin/main HEAD_REF=HEAD ./scripts/check-npm-lock-heuristics.py
./scripts/test-npm-lock-heuristics.sh
```

## CI isolation

Dependency PR workflows that check out Dependabot code use **read-only**
`permissions` and do **not** receive repository secrets:

| Workflow | Trigger | Notes |
| --- | --- | --- |
| `dependabot-review-gate.yml` | `pull_request` | `contents: read`, `pull-requests: read` |
| `dependabot-dep-scan.yml` | `pull_request` | same |
| `dependabot-auto-merge.yml` | `pull_request_target` | **does not check out PR code**; only metadata + `gh pr merge` |

Release / publish (`release.yml`) runs only from **tags** (or manual dispatch)
on trusted refs and holds `packages: write`, `id-token: write`, and
`attestations: write` for signing — never from Dependabot PR events.

## Signed releases (cosign + SBOM)

Published GHCR images are:

1. Built with Buildx **provenance** + **SBOM**
2. **Cosign-signed** keyless (GitHub Actions OIDC → Fulcio; identity is
   `release.yml` on this repo’s `v*` tags)
3. Attested with GitHub **build provenance** and **SPDX SBOM** (pushed to the
   registry and attached to the GitHub Release as `*.spdx.json`)

### Operator verify (opt-in)

Default upgrades stay offline-friendly (no cosign required). To enforce
signatures after a GHCR pull:

```bash
# Flag
./scripts/install.sh upgrade --yes --version 0.7.0 --verify-images
# or env (also synced as missing key from .env.example)
VULYK_VERIFY_IMAGES=1 ./scripts/install.sh upgrade --yes --version 0.7.0

# Standalone
./scripts/verify-release-images.sh 0.7.0
```

Install [cosign](https://docs.sigstore.dev/cosign/installation/) first.
Override identity / issuer with `VULYK_COSIGN_IDENTITY_REGEXP` /
`VULYK_COSIGN_OIDC_ISSUER` if you fork the Release workflow.

Proof (no network / no cosign binary):

```bash
./scripts/test-verify-release-images.sh
```

## Dev Container

[`.devcontainer/`](../../.devcontainer/) gives contributors a **host-isolated**
toolchain (Rust stable + rustfmt/clippy, Node 22, Docker Compose via
docker-outside-of-docker, GitHub CLI). Open the repo in VS Code / Cursor
“Reopen in Container”, or Codespaces.

`post-create.sh` installs components, sets `scripts/githooks`, and runs
`./scripts/install.sh check`. Use the same secure install helpers as CI
(`./scripts/npm-ci-secure.sh`). Validation:

```bash
./scripts/check-devcontainer.sh
```

## Related

- [ci-checks.md](ci-checks.md) - workflow index
- [releasing.md](releasing.md) - tags / GHCR / verify
- [upgrade.md](../guides/upgrade.md) - `--verify-images` on upgrade
- [CONTRIBUTING.md](../../CONTRIBUTING.md) - local / Dev Container setup
