# PWA and mobile

Vulyk’s web UI (`frontend/`) is a Progressive Web App and is designed for phone use during manual test execution.

## Install

1. Open the deployed UI over HTTPS (or `localhost` for local preview).
2. Use the browser install affordance, or the in-app **Install** toast when Chromium fires `beforeinstallprompt`.
3. The app opens standalone with bee-themed icons (any + maskable sizes 72–512).

## Offline behaviour

| Capability | Behaviour |
|---|---|
| App shell | Service worker (Workbox via `vite-plugin-pwa`) precaches the built shell |
| Offline page | `/offline.html` is cached for hard navigations when the shell is unavailable |
| Recently viewed cases/runs | IndexedDB (`vulyk-offline`) stores the last ~40 cases and ~20 runs on view |
| API GETs | Network-first runtime cache for `/api/v1/**/cases|runs|suites` |
| Step results while offline | Queued in IndexedDB and flushed on `online` (banner shows pending count) |
| Updates | `registerType: 'prompt'` — toast offers **Update** when a new SW waits |

Recording a pass/fail/blocked/skipped result on a run while offline enqueues the payload and shows a notice. When connectivity returns, the queue syncs automatically (or via **Sync now**).

## Mobile UX

- **Bottom navigation** (Cases / Runs / Reports / Admin) on viewports ≤860px
- **Suite tree drawer** — open with the **Suites** control in the top bar
- Case list → detail stacks full-width; tables scroll horizontally when needed
- **Run execution** is one-handed: case queue list → single-step card → large **Passed / Failed / Blocked / Skipped** buttons near the thumb; camera capture for result attachments (`capture="environment"`)
- Admin tabs and report tables use larger tap targets and stacked grids

## Local development

```bash
cd frontend
npm ci
npm run icons          # regenerate bee PNG icons (optional)
npm run build          # emits SW + manifest
npm run preview -- --host 127.0.0.1 --port 5173
```

Service worker registration is disabled in `vite` dev (`devOptions.enabled: false`) so HMR stays simple. Use **preview** (or the nginx Docker image) to exercise PWA behaviour.

## CI checks

- **Playwright** (`e2e/`): Desktop Chromium plus **iPhone 13** and **Pixel 5** projects (`mobile.spec.ts` covers bottom nav, suite drawer, manifest/SW).
- **Lighthouse**: `frontend/scripts/lighthouse-ci.mjs` asserts accessibility ≥ 0.9 and PWA audits (`installable-manifest`, `service-worker`, `maskable-icon`, `viewport`, splash/theme).

Workflow jobs: `Playwright e2e` and `Lighthouse PWA/a11y` in `.github/workflows/ci.yml`.

## Nginx notes

`frontend/nginx.conf` serves `sw.js` and `manifest.webmanifest` with `Cache-Control: no-cache` so clients pick up updates promptly. HTML routes still fall back to `index.html` for the SPA.

## Related code

- `frontend/vite.config.ts` — `VitePWA` / Workbox config
- `frontend/src/pwa/` — SW registration, IndexedDB cache, offline queue sync
- `frontend/src/components/Layout.tsx`, `BottomNav.tsx`, `PwaToasts.tsx`
- `frontend/src/pages/RunDetail.tsx` — mobile execution + offline queue + camera
