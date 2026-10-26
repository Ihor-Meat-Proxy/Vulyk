# Changelog

All notable changes to Vulyk are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Hardened upgrade backups: verified temp-DB restore, config/secrets bundle,
  attachments archive, auto-rollback to previous images, downgrade guards
- Upgrade CI asserts encrypted LLM secrets decrypt after migrate

## [0.1.0] - 2026-10-26

Initial open-source release of Vulyk (*вулик* — Ukrainian for *beehive*).

### Added

- **API** — Rust (Axum + SQLx + OpenAPI/Scalar): auth/RBAC, personal API tokens,
  projects, nested folders, auto case keys (`PREFIX-N`), plans, runs, step results,
  requirements links, reports, CI ingest (JUnit / Playwright / pytest / Vulyk JSON),
  automation executions (campaigns, iterations, pipeline upload), multi-repo CI
  dispatch/webhooks, LLM generate + chat propose/create, importers scaffold
- **UI** — React + TypeScript + Vite PWA: cases/plans/runs, folder tree, i18n
  (en/uk/fr/es/de), theming, mobile bottom nav, offline queue
- **MCP** — `vulyk-mcp` stdio/HTTP server for AI assistants
- **CLI** — `vulyk` reporter for CI result upload
- **Deploy** — Docker Compose (dev + prod), Helm chart, `./scripts/install.sh`
  (`compose` / `prod` / `dev` / `setup` / `upgrade` / `backup` / `restore`)
- **Upgrades** — `vulyk-api migrate status|run`, advisory locks, downgrade
  protection, `VULYK_AUTO_MIGRATE`, Helm pre-upgrade migrate Job, PVC keep policy
- **Versioning** — `/health` and `/ready` expose `version`; UI sidebar shows API
  semver; GitHub Releases + GHCR images via `.github/workflows/release.yml`
- **Docs** — architecture, admin/user guides, upgrade and releasing guides

### Release artifacts

- GHCR images: `vulyk-api` and `vulyk-web` tagged `0.1.0` and `latest`
- GitHub Release attaches Linux API binary, frontend `dist` archive, Compose/install
  bundle, and SHA256 checksums
