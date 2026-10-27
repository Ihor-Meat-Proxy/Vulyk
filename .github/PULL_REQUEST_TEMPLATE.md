## Summary

<!-- What does this PR change and why? -->

## Upgrade path

Does this PR affect upgrading an **existing** install (new/renamed env or config
keys, Compose services/volumes/images, DB schema/data, on-disk layout,
`install.sh` upgrade behavior)?

- [ ] **No** — no operator upgrade impact
- [ ] **Yes** — same PR updates (all that apply):
  - [ ] `.env.example` sync markers (`@vulyk:sync` / `@vulyk:secret` / `@vulyk:required`)
  - [ ] `./scripts/install.sh` upgrade / preflight (stopped API tolerated)
  - [ ] SQL migration(s) (forward-only; do not edit migrations already on `main`)
  - [ ] `./scripts/ci-env-upgrade-test.sh` still passes
  - [ ] `docs/guides/upgrade.md` (if operator steps changed)

Existing operator `.env` values, secrets, and admin passwords must **not** be
overwritten — only missing keys may be appended (with backup).

## Test plan

- [ ] `./scripts/check.sh`
- [ ] Relevant unit/integration/e2e tests
- [ ] CI green
