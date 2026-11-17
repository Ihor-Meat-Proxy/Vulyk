# Supply-chain policy

How Vulyk keeps dependencies fresh without blindly trusting brand-new or
hijacked releases. Tracking: [#294](https://github.com/Ihor-Meat-Proxy/Vulyk/issues/294).

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

## Signed releases (cosign + SBOM)

From **v0.7.0** onward (Release workflow on this branch), published GHCR images are:

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

Install [cosign](https://docs.sigstore.dev/cosign/system_config/installation/)
first. Override identity / issuer with `VULYK_COSIGN_IDENTITY_REGEXP` /
`VULYK_COSIGN_OIDC_ISSUER` if you fork the Release workflow.

Proof (no network / no cosign binary):

```bash
./scripts/test-verify-release-images.sh
```

## Related

- [ci-checks.md](ci-checks.md) - workflow index
- [releasing.md](releasing.md) - tags / GHCR / verify
- [upgrade.md](../guides/upgrade.md) - `--verify-images` on upgrade
