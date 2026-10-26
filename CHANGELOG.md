# Changelog

All notable changes to Vulyk are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Safe Compose upgrades via `./scripts/install.sh upgrade|backup|restore`
- `vulyk-api migrate status|run` and `vulyk-api version`, with advisory locks and downgrade protection
- `VULYK_AUTO_MIGRATE` (default true in dev Compose, false in prod / Helm)
- Helm pre-upgrade migrate Job, API migrate initContainer, PVC `helm.sh/resource-policy: keep`
- `/health` and `/ready` expose `version`; UI sidebar shows API version
- Migration integrity CI check and old→new upgrade CI job
- Operator guide: [docs/guides/upgrade.md](docs/guides/upgrade.md)

## [0.1.0] - 2026-10-26

### Added

- Initial open-source release: Rust API, React UI, MCP, CLI, Docker Compose, Helm
