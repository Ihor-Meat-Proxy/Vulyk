# Design tokens & appearance

Vulyk uses CSS custom properties (design tokens) for colour, spacing, and typography. Components must not hardcode theme colours — use `var(--…)` (or status badge components).

## Themes

| Choice | Resolved | Notes |
|--------|----------|--------|
| `system` (default) | `light` / `dark` / `high_contrast` | Follows `prefers-color-scheme`; upgrades to high contrast when `prefers-contrast: more` |
| `light` | light | Page bg ≈ `#F7F8FA`, text ≈ `#1F2328` (no pure white/black) |
| `dark` | dark | Page bg ≈ `#121417`, text ≈ `#E6E6E6`, desaturated accents |
| `high_contrast` | high contrast | WCAG AAA ≥ **7:1** for text pairs; also used under `forced-colors` |
| `sepia` | sepia / night warm | Low-blue warm palette for evening use |

Resolved theme is set on `<html data-theme="…">`. The user’s choice is `data-theme-choice`. Density is `data-density="comfortable|compact"`. Font scale is `--font-scale` (percent / 100). UI body font is `data-ui-font` / `--font-body` (self-hosted `@fontsource` families; default Inter).

Token source of truth for tests: [`frontend/src/theme/tokens.ts`](../frontend/src/theme/tokens.ts). Runtime apply: [`frontend/src/theme/applyAppearance.ts`](../frontend/src/theme/applyAppearance.ts). An inline script in `frontend/index.html` reads `localStorage` key `vulyk.appearance` before paint to avoid a wrong-theme flash.

## Core tokens

| Token | Role |
|-------|------|
| `--bg`, `--bg-elevated` | Page / panels |
| `--ink`, `--ink-muted` | Text |
| `--line` | Borders / dividers |
| `--accent`, `--accent-soft`, `--accent-on` | Brand actions |
| `--warn`, `--danger` | Non-status warnings / errors |
| `--focus-ring` | Focus outline |
| `--theme-color` | PWA / `theme-color` meta |
| `--code-bg`, `--code-ink` | Code blocks |
| `--surface-input` | Form controls |
| `--font-scale`, `--space-*`, `--control-pad-*` | Type scale & density |

## Status palette (Okabe–Ito)

Result statuses always use **icon + label** (never colour alone). Charts/progress bars add hatch patterns and a text legend.

| Status | Hue (Okabe–Ito family) | Icon |
|--------|------------------------|------|
| Passed / Completed / Succeeded / Closed | Bluish green `#009E73` | Check |
| Failed | Vermillion `#D55E00` | X |
| Blocked | Purple / reddish purple | Block |
| Skipped / Cancelled | Grey | Skip / cancel |
| Retest | Amber / orange `#E69F00` | Retry |
| Ready / Open / Queued | Sky blue | Filled / hollow circle |
| Running / In progress | Blue `#0072B2` | Spinner / clock |
| Untested / Draft | Outline / muted | Hollow circle |
| Untestable | Slate dashed | Ban |

Use the shared `StatusBadge` / `StatusIcon` components everywhere (Runs list, executions dashboard, automation suites, run details, per-case executions). Never render raw enum strings in a generic `.badge`.

Per-theme badge pairs: `--status-*-fg` / `--status-*-bg` / solid `--status-*` for charts.

## Contrast policy

- All themes: body and badge text pairs ≥ **4.5:1** (WCAG AA).
- `high_contrast`: ≥ **7:1** (AAA).
- Enforced by unit test: `frontend/src/theme/contrast.test.ts`.

## User & instance preferences

- **User appearance:** gear → **Appearance**, or `PATCH /api/v1/auth/me` with `preferences: { theme, font_size, font_family, density, reduced_motion }` (stored in `users.preferences` JSONB). `font_family` is one of `inter` (default), `ibm_plex_sans`, `noto_sans`, `source_sans_3`.
- **User locale:** globe menu or gear → **My settings**; `PATCH /api/v1/auth/me` with `locale` (`en` / `uk` / `fr` / `es` / `de`). Anonymous sessions keep `localStorage['vulyk.locale']`.
- **Instance defaults:** gear → **Admin settings** → default theme / locale (`system_settings`); also on `GET /api/v1/auth/status` as `default_theme` / `default_locale`.
- **Local appearance:** `localStorage['vulyk.appearance']` for pre-login and instant apply.
- **Reduced motion:** user override or `prefers-reduced-motion: reduce` → `data-reduced-motion="true"`.

Missing UI translation keys fall back to English, then to the key string. Seeded sample content uses bilingual fields with English fallback for `fr` / `es` / `de` when those locale strings are empty.

See [user-guide.md](./user-guide.md#appearance) for the UI walkthrough.
