/**
 * Lighthouse accessibility gate + static PWA installability checks.
 *
 * Lighthouse ≥12 removed the scored `pwa` category (and installable-manifest /
 * maskable-icon / splash-screen / themed-omnibox audits). We still assert a11y
 * via Lighthouse and cover PWA basics with `assertPwaArtifacts` against the
 * preview server (security audit M14 / lighthouse ≥13.5).
 *
 * Expects the preview server at LIGHTHOUSE_URL (default http://127.0.0.1:5173).
 */
import { spawn } from 'node:child_process'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { readFile } from 'node:fs/promises'
import { assertPwaArtifacts } from './pwa-gates.mjs'

const require = createRequire(import.meta.url)
const __dirname = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(__dirname, '..')

const BASE_URL = process.env.LIGHTHOUSE_URL ?? 'http://127.0.0.1:5173'
const MIN_A11Y = Number(process.env.LH_MIN_A11Y ?? '0.85')

async function waitForServer(url, attempts = 60) {
  for (let i = 0; i < attempts; i++) {
    try {
      const res = await fetch(url)
      if (res.ok || res.status === 200) return
    } catch {
      /* retry */
    }
    await new Promise((r) => setTimeout(r, 500))
  }
  throw new Error(`Server not ready at ${url}`)
}

function run(cmd, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, {
      stdio: ['ignore', 'pipe', 'pipe'],
      env: process.env,
    })
    let stderr = ''
    child.stdout.on('data', (d) => process.stdout.write(d))
    child.stderr.on('data', (d) => {
      stderr += d
      process.stderr.write(d)
    })
    child.on('error', reject)
    child.on('close', (code) => {
      if (code === 0) resolve()
      else reject(new Error(`${cmd} exited ${code}\n${stderr}`))
    })
  })
}

await waitForServer(BASE_URL)

let lighthouseBin
try {
  lighthouseBin = require.resolve('lighthouse/cli/index.js')
} catch {
  console.error('lighthouse is not installed')
  process.exit(1)
}

const lhPkg = require('lighthouse/package.json')
const lhMajor = Number.parseInt(String(lhPkg.version).split('.')[0], 10)
if (!Number.isFinite(lhMajor) || lhMajor < 13) {
  console.error(`lighthouse ${lhPkg.version} is below required ≥13.5 (M14)`)
  process.exit(1)
}
console.log(`lighthouse ${lhPkg.version}`)

const reportPath = path.join(root, 'lighthouse-report.json')
const chromeFlags = [
  '--headless=new',
  '--no-sandbox',
  '--disable-dev-shm-usage',
]
if (process.env.CHROME_PATH) {
  // lighthouse picks CHROME_PATH from env automatically via chrome-launcher
}

await run(process.execPath, [
  lighthouseBin,
  BASE_URL,
  // PWA category removed in LH 12; unknown onlyCategories is an error in LH 13.
  '--only-categories=accessibility',
  `--chrome-flags=${chromeFlags.join(' ')}`,
  '--output=json',
  `--output-path=${reportPath}`,
  '--quiet',
  '--form-factor=mobile',
])

const report = JSON.parse(await readFile(reportPath, 'utf8'))
const cats = report.categories ?? {}
const a11y = cats.accessibility?.score ?? 0

console.log(`Lighthouse accessibility score: ${a11y}`)

let failed = false
if (a11y < MIN_A11Y) {
  console.error(`Accessibility score ${a11y} below minimum ${MIN_A11Y}`)
  failed = true
}

const pwa = await assertPwaArtifacts(BASE_URL)
for (const n of pwa.notes) console.log(`PWA: ${n}`)
if (!pwa.ok) {
  for (const f of pwa.failures) console.error(`PWA FAIL: ${f}`)
  failed = true
} else {
  console.log('PWA static installability checks passed')
}

if (failed) process.exit(1)
console.log('Lighthouse accessibility + PWA checks passed')
