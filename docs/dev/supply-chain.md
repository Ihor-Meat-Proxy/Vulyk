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

## Related

- [ci-checks.md](ci-checks.md) — workflow index
- [releasing.md](releasing.md) — tags / GHCR (signing / SBOM land with later #294 slices)
