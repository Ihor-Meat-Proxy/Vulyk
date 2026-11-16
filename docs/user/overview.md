# Overview

Vulyk is a self-hosted **test management system** (TMS) for manual and automated testing. You organise cases into nested folders, plan and execute runs, ingest CI results by case key, link requirements, and review dashboards and reports.

## What you can do

- Organise **projects**, **folders**, and **test cases** with auto-allocated keys, tags, and custom fields
- Link **requirements** to cases and review **coverage** under Reports
- Create **runs**, record step results and attachments, and browse **Executions** (campaigns → iterations → automated suites)
- Connect **CI** reporters (JUnit, Playwright, pytest, Vulyk JSON) and **automation Git repositories**
- Use optional **AI** assist to generate cases or automation stubs and to chat over the project suite (when an admin configures a provider)
- Administer users, appearance, languages, content backups, and in-app or CLI upgrades

## Main areas in the app

| Area | Route | Purpose |
|------|-------|---------|
| Cases | `/` | Folder tree + case list |
| Case detail | `/cases/{KEY}` | Edit case, history, executions, PDF |
| Requirements | `/requirements` | Requirements list and custom fields |
| Import | `/import` | Import cases from other tools / CSV |
| Generate | `/generate` | AI case and automation generation |
| Chat | `/chat` | AI Q&A and propose/create drafts |
| Runs | `/runs`, `/runs/{id}` | Plans, runs, manual execution |
| Executions | `/executions` | Campaigns, iterations, CI suites |
| CI (legacy entry) | `/ci` | Same run/CI workspace as Runs CI tooling |
| Reports | `/reports`, `/reports/runs/{id}`, `/reports/coverage` | Dashboards, run reports, coverage |
| Admin | `/admin` | Users, settings, updates, lists, LLM, … |
| Appearance | `/appearance` | Theme, fonts, density, motion |
| Settings | `/settings` | Profile, tokens, CI tokens, automation repos |

## Sign in

1. Open the UI for your instance.
2. On a fresh install, complete **first-run setup** (create the first system admin), then **Sign in** with email and password.
3. Personal API tokens for CLI/MCP live under the gear menu → **My settings** (`/settings`). Admins can also manage tokens under **Admin → Tokens**.

When authentication is intentionally disabled (`VULYK_AUTH_REQUIRED=false`), the UI runs in **Open mode** (badge in the gear menu). New installs default to auth required; production markers refuse to boot with auth off.

Password policy is server-enforced (default minimum length 16, letter + digit). After login, if your password no longer meets policy, you must change it before other API routes work.

## Language

Use the **globe** icon in the header to switch among English, Українська, Français, Español, and Deutsch. The choice is saved on your account when signed in (and in browser storage for anonymous / open-mode sessions).

## Demo sample data

On demo/dev installs with sample seeding enabled, Vulyk can load two sample projects (marked **Sample**):

| Project | Key | Case prefix |
|---------|-----|-------------|
| Kyiv Open-Air Concert 2026 | `KYIV_CONCERT_2026` | `KC` |
| Vulyk QA | `VULYK_QA` | `VQA` |

System admins can remove or re-seed sample data under **Admin → Settings → Sample data**. Only rows flagged as sample are deleted; your own projects are never removed by that action.

## First-time setup

If this is a new install:

1. Prefer **env seeding** or `./scripts/install.sh` / Compose / Helm so the first admin and instance defaults are created automatically.
2. Or complete the **web wizard** with the one-time setup token printed in the API logs (`vulyk_setup_…`).
3. Or run `vulyk-api setup` / `./scripts/install.sh setup`.

Operators configure instance name, default locale (`en` / `uk` / `fr` / `es` / `de`), timezone, and registration mode (`open` / `invite` / `disabled`) during setup. After setup completes, sign in and pick a project from the top-bar switcher.

## How to use this documentation

Pick a topic from the docs viewer sidebar. Page anchors use kebab-case headings so deep links stay stable. This help set describes in-app product flows; operator install and upgrade details also appear under **Admin** when relevant.
