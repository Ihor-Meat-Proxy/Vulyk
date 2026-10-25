/**
 * Lighthouse PWA + accessibility gates (Lighthouse 11 still ships a scored PWA category).
 * Expects the preview server at LIGHTHOUSE_URL (default http://127.0.0.1:5173).
 */
import { spawn } from 'node:child_process'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { readFile } from 'node:fs/promises'

const require = createRequire(import.meta.url)
const __dirname = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(__dirname, '..')

const BASE_URL = process.env.LIGHTHOUSE_URL ?? 'http://127.0.0.1:5173'
const MIN_PWA = Number(process.env.LH_MIN_PWA ?? '0.8')
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
  '--only-categories=pwa,accessibility',
  `--chrome-flags=${chromeFlags.join(' ')}`,
  '--output=json',
  `--output-path=${reportPath}`,
  '--quiet',
  '--form-factor=mobile',
])

const report = JSON.parse(await readFile(reportPath, 'utf8'))
const cats = report.categories ?? {}
const pwa = cats.pwa?.score
const a11y = cats.accessibility?.score ?? 0

console.log(`Lighthouse PWA score: ${pwa}`)
console.log(`Lighthouse accessibility score: ${a11y}`)

// Lighthouse 11 folds the service-worker check into installable-manifest.
const required = [
  'installable-manifest',
  'maskable-icon',
  'viewport',
  'splash-screen',
  'themed-omnibox',
]
let failed = false
for (const id of required) {
  const audit = report.audits?.[id]
  if (!audit) {
    console.error(`Missing audit: ${id}`)
    failed = true
    continue
  }
  console.log(`${id}: score=${audit.score} — ${audit.title}`)
  if (typeof audit.score === 'number' && audit.score < 1) {
    console.error(`  FAIL details: ${JSON.stringify(audit.details ?? audit.explanation ?? '').slice(0, 600)}`)
    failed = true
  }
}

if (typeof pwa !== 'number' || pwa < MIN_PWA) {
  console.error(`PWA score ${pwa} below minimum ${MIN_PWA}`)
  failed = true
}
if (a11y < MIN_A11Y) {
  console.error(`Accessibility score ${a11y} below minimum ${MIN_A11Y}`)
  failed = true
}

if (failed) process.exit(1)
console.log('Lighthouse PWA + accessibility checks passed')
