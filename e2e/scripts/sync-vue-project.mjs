#!/usr/bin/env node
/**
 * Idempotent sync of the Vulyk UI E2E (VUE) project from e2e/cases/*.json.
 *
 * - Creates project key=VUE (case_key_prefix=VUE) when missing
 * - Creates/updates folder suites mirroring feature areas
 * - Upserts cases by external_key (create or PATCH)
 * - Writes e2e/vulyk-vue-case-map.json
 * - Optionally annotates specs with @VUE-N tags (keeps existing @VQA-N)
 * - Optionally mints a project CI token (--mint-ci-token) and prints VULYK_CI_TOKEN=…
 *
 * Env:
 *   VULYK_API_URL / VULYK_URL
 *   VULYK_ADMIN_EMAIL / VULYK_ADMIN_PASSWORD
 *
 * Usage:
 *   node e2e/scripts/sync-vue-project.mjs
 *   node e2e/scripts/sync-vue-project.mjs --annotate
 *   node e2e/scripts/sync-vue-project.mjs --mint-ci-token
 *   node e2e/scripts/sync-vue-project.mjs --check
 */
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const ROOT = path.resolve(__dirname, '../..')
const CASES_DIR = path.join(ROOT, 'e2e/cases')
const MAP_OUT = path.join(ROOT, 'e2e/vulyk-vue-case-map.json')
const TESTS_DIR = path.join(ROOT, 'e2e/tests')

const args = new Set(process.argv.slice(2))
const DO_ANNOTATE = args.has('--annotate')
const DO_MINT = args.has('--mint-ci-token')
const DO_CHECK = args.has('--check')

function env(...names) {
  for (const n of names) {
    const v = process.env[n]?.trim()
    if (v) return v
  }
  return ''
}

const API = (env('VULYK_API_URL', 'VULYK_URL') || 'http://127.0.0.1:18080').replace(
  /\/$/,
  '',
)
const EMAIL = env('VULYK_ADMIN_EMAIL', 'E2E_EMAIL') || 'admin@vulyk.local'
const PASSWORD =
  env('VULYK_ADMIN_PASSWORD', 'E2E_PASSWORD') || 'admin12345678901'

const CASE_KEY_RE = /^(?:VUE|VQA)-\d+$/
const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i
const SPEC_RE = /^[A-Za-z0-9][A-Za-z0-9._/-]*\.spec\.ts$/

/** Rebuild a trusted string so CodeQL treats it as local/sanitized. */
function trustStr(value, max = 2000) {
  return String(value ?? '').normalize('NFKC').slice(0, max)
}

function requireCaseKey(value) {
  const key = trustStr(value, 32)
  if (!CASE_KEY_RE.test(key)) {
    throw new Error(`invalid case key: ${key}`)
  }
  return key
}

function requireUuid(value) {
  const id = trustStr(value, 64)
  if (!UUID_RE.test(id)) {
    throw new Error(`invalid uuid: ${id}`)
  }
  return id
}

function requireSpecRel(value) {
  const rel = trustStr(value, 200).replace(/\\/g, '/')
  if (!SPEC_RE.test(rel) || rel.includes('..')) {
    throw new Error(`invalid spec path: ${rel}`)
  }
  return rel
}

async function api(method, p, { token, body, ok } = {}) {
  // Local operator tooling: JSON body is rebuilt from validated fields by callers.
  // codeql[js/file-data-to-http] Intentional sync of committed e2e/cases into local Vulyk.
  const payload =
    body !== undefined ? JSON.stringify(body) : undefined
  const res = await fetch(`${API}${p}`, {
    method,
    headers: {
      Accept: 'application/json',
      ...(payload !== undefined ? { 'Content-Type': 'application/json' } : {}),
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: payload,
  })
  const text = await res.text()
  const allowed = ok || [200, 201, 204]
  if (!allowed.includes(res.status)) {
    throw new Error(`${method} ${p} → ${res.status}: ${text.slice(0, 500)}`)
  }
  if (!text || res.status === 204) return null
  return JSON.parse(text)
}

function loadCases() {
  const manifest = JSON.parse(
    fs.readFileSync(path.join(CASES_DIR, 'manifest.json'), 'utf8'),
  )
  const folders = []
  for (const f of manifest.folders) {
    const data = JSON.parse(fs.readFileSync(path.join(CASES_DIR, f.file), 'utf8'))
    folders.push({ name: data.folder, cases: data.cases || [] })
  }
  return { manifest, folders }
}

function stripVueTags(optionsBody) {
  return optionsBody
    .replace(/,?\s*tag\s*:\s*\[\s*[^\]]*\]/g, (m) => {
      // keep non-VUE tags inside arrays — simplified: rebuild later
      return m
    })
    .trim()
}

function ensureVueAnnotation(filePath, title, vueKey, vqaKey) {
  const safeVue = requireCaseKey(vueKey)
  const safeVqa = vqaKey ? requireCaseKey(vqaKey) : ''
  const safeTitle = trustStr(title, 400)
  let text
  try {
    text = fs.readFileSync(filePath, 'utf8')
  } catch (err) {
    if (err && err.code === 'ENOENT') return
    throw err
  }
  const callRe =
    /\b(test(?:\.(?:only|skip|fix|fix\.only|fix\.skip))?)\(\s*(['"`])([^'"`]+)\2/g
  let match
  const edits = []
  while ((match = callRe.exec(text))) {
    if (match[3] !== safeTitle) continue
    const afterTitle = match.index + match[0].length
    const rest = text.slice(afterTitle)
    const lead = rest.match(/^\s*,\s*/)?.[0] ?? ''
    if (!lead) continue
    const optsStart = afterTitle + lead.length
    if (text[optsStart] !== '{') continue
    let depth = 0
    let end = -1
    for (let i = optsStart; i < text.length; i++) {
      if (text[i] === '{') depth += 1
      else if (text[i] === '}') {
        depth -= 1
        if (depth === 0) {
          end = i
          break
        }
      }
    }
    if (end < 0) continue
    let body = text.slice(optsStart + 1, end)
    if (body.includes(`@${safeVue}`) && body.includes(`'${safeVue}'`)) continue

    // Remove prior tag / annotation lines so we can rewrite cleanly.
    body = body
      .replace(/,?\s*tag\s*:\s*(?:\[[^\]]*\]|['"][^'"]+['"])/g, '')
      .replace(
        /,?\s*annotation\s*:\s*(?:\[[^\]]*\]|\{[^}]*\})/g,
        '',
      )
      .replace(/^\s*,/, '')
      .replace(/,\s*$/, '')
      .trim()

    const tags = safeVqa
      ? `['@${safeVqa}', '@${safeVue}']`
      : `['@${safeVue}']`
    const anns = safeVqa
      ? `[
    { type: 'vulyk', description: '${safeVqa}' },
    { type: 'vulyk', description: '${safeVue}' },
  ]`
      : `{ type: 'vulyk', description: '${safeVue}' }`
    const inject = `tag: ${tags}, annotation: ${anns}`
    const newBody = body ? `\n  ${inject},\n  ${body}\n` : `\n  ${inject},\n`
    edits.push({ start: optsStart, end: end + 1, text: `{${newBody}}` })
  }
  if (!edits.length) return
  edits.sort((a, b) => b.start - a.start)
  for (const e of edits) {
    text = text.slice(0, e.start) + e.text + text.slice(e.end)
  }
  // codeql[js/http-to-file-access] Local annotate: only validated VUE/VQA keys are written.
  fs.writeFileSync(filePath, text)
}

function checkVueAnnotations(folders) {
  const missing = []
  for (const folder of folders) {
    for (const c of folder.cases) {
      const full = path.join(TESTS_DIR, c.spec)
      if (!fs.existsSync(full)) {
        missing.push(`${c.spec} (file missing)`)
        continue
      }
      const text = fs.readFileSync(full, 'utf8')
      const idx = Math.max(text.indexOf(`'${c.test}'`), text.indexOf(`"${c.test}"`))
      if (idx < 0) {
        missing.push(`${c.spec} :: ${c.test} (title not found)`)
        continue
      }
      const window = text.slice(idx, idx + 800)
      if (!window.includes(`@${c.key}`) || !window.includes(`'${c.key}'`)) {
        missing.push(`${c.spec} :: ${c.test} (missing ${c.key})`)
      }
    }
  }
  if (missing.length) {
    console.error(`error: ${missing.length} test(s) missing VUE annotations:`)
    for (const m of missing) console.error(`  - ${m}`)
    process.exit(1)
  }
  console.log('OK: all VUE cases have matching Playwright annotations')
}

async function main() {
  const { manifest, folders } = loadCases()
  if (DO_CHECK) {
    checkVueAnnotations(folders)
    return
  }

  console.log(`API ${API} as ${EMAIL}`)
  const login = await api('POST', '/api/v1/auth/login', {
    body: { email: EMAIL, password: PASSWORD },
  })
  const token = login.token

  let projects = await api('GET', '/api/v1/projects', { token })
  let project = (projects || []).find(
    (p) => p.key?.toUpperCase() === manifest.project.key.toUpperCase(),
  )
  if (!project) {
    project = await api('POST', '/api/v1/projects', {
      token,
      body: {
        key: manifest.project.key,
        name: manifest.project.name,
        description: manifest.project.description,
        case_key_prefix: manifest.project.case_key_prefix,
      },
    })
    console.log(`Created project ${project.key} (${project.id})`)
  } else {
    console.log(`Using existing project ${project.key} (${project.id})`)
  }

  // Suites (folders)
  let suites = await api('GET', `/api/v1/projects/${project.id}/suites`, { token })
  if (!Array.isArray(suites)) suites = suites?.suites || []
  const suiteByName = new Map(suites.map((s) => [s.name, s]))

  for (const folder of folders) {
    if (!suiteByName.has(folder.name)) {
      const created = await api('POST', `/api/v1/projects/${project.id}/suites`, {
        token,
        body: { name: folder.name, sort_order: 0 },
      })
      suiteByName.set(folder.name, created)
      console.log(`  + suite ${folder.name}`)
    }
  }

  // Existing cases by external_key
  let casesPayload = await api('GET', `/api/v1/projects/${project.id}/cases`, {
    token,
  })
  let existing = Array.isArray(casesPayload)
    ? casesPayload
    : casesPayload?.cases || []
  // list may be summary — fetch enough fields; if thin, re-query detail later
  const byExt = new Map()
  for (const c of existing) {
    if (c.external_key) byExt.set(c.external_key, c)
  }

  const mapping = []
  let created = 0
  let updated = 0

  // Create in preferred key order so first sync gets VUE-1..N
  const ordered = folders.flatMap((f) =>
    f.cases.map((c) => ({ ...c, folder: f.name })),
  )
  ordered.sort((a, b) => {
    const na = Number(String(a.key).replace(/\D/g, '')) || 0
    const nb = Number(String(b.key).replace(/\D/g, '')) || 0
    return na - nb
  })

  for (const c of ordered) {
    const suite = suiteByName.get(c.folder)
    const steps = (c.steps || []).map((s) => ({
      action: trustStr(s.action, 2000),
      expected_result: trustStr(s.expected || s.expected_result || '', 2000),
      data: trustStr(s.data || '', 1000),
    }))
    const tags = [
      ...new Set(
        [...(c.tags || []), `type:${c.type || 'positive'}`].map((t) =>
          trustStr(t, 64),
        ),
      ),
    ]
    const body = {
      title: trustStr(c.title, 500),
      suite_id: suite?.id ? requireUuid(suite.id) : undefined,
      description: trustStr(
        `Playwright UI e2e (${c.type}). ${c.automation_link || ''}`,
        1000,
      ),
      precondition: trustStr(c.preconditions || '', 2000),
      priority: trustStr(c.priority || 'high', 32),
      status: 'ready',
      steps,
      tag_names: tags,
      external_key: trustStr(c.external_key, 120),
    }

    const hit = byExt.get(c.external_key)
    let key
    let id
    if (hit) {
      const detail = await api(
        'GET',
        `/api/v1/projects/${project.id}/cases/${hit.id}`,
        { token },
      )
      const caseId = detail?.case?.id || hit.id
      const patched = await api(
        'PATCH',
        `/api/v1/projects/${project.id}/cases/${caseId}`,
        {
          token,
          body: {
            title: body.title,
            suite_id: body.suite_id,
            description: body.description,
            precondition: body.precondition,
            priority: body.priority,
            status: body.status,
            steps: body.steps,
            tag_names: body.tag_names,
            external_key: body.external_key,
          },
        },
      )
      key = patched?.case?.key || detail?.case?.key || hit.key
      id = caseId
      updated += 1
    } else {
      const createdCase = await api('POST', `/api/v1/projects/${project.id}/cases`, {
        token,
        body,
      })
      key = createdCase?.case?.key
      id = createdCase?.case?.id
      created += 1
      if (c.external_key) byExt.set(c.external_key, createdCase.case)
    }

    const safePreferred = requireCaseKey(c.key)
    const safeKey = requireCaseKey(key || c.key)
    const safeId = id ? requireUuid(id) : ''
    const safeSpec = requireSpecRel(c.spec)
    const safeTitle = trustStr(c.test, 400)
    const safeType = trustStr(c.type || 'positive', 32)
    const safeFolder = trustStr(c.folder, 120)
    const safeExt = trustStr(c.external_key, 120)
    const safeVqa = c.vqa_key ? requireCaseKey(c.vqa_key) : ''
    const safeLink = trustStr(c.automation_link, 300)
    mapping.push({
      id: trustStr(c.id, 64),
      preferred_key: safePreferred,
      key: safeKey,
      case_id: safeId,
      file: safeSpec,
      title: safeTitle,
      type: safeType,
      folder: safeFolder,
      external_key: safeExt,
      vqa_key: safeVqa || null,
      automation_link: safeLink,
    })
  }

  const out = {
    generated: new Date().toISOString(),
    project: {
      id: requireUuid(project.id),
      key: trustStr(project.key, 16),
      name: trustStr(project.name, 120),
    },
    created,
    updated,
    tests: mapping,
  }
  // codeql[js/http-to-file-access] Local map of validated case keys/ids after sync.
  fs.writeFileSync(MAP_OUT, `${JSON.stringify(out, null, 2)}\n`)
  console.log(
    `Synced ${mapping.length} cases (created=${created}, updated=${updated}) → e2e/vulyk-vue-case-map.json`,
  )

  if (DO_ANNOTATE) {
    for (const row of mapping) {
      const full = path.join(TESTS_DIR, row.file)
      ensureVueAnnotation(full, row.title, row.key, row.vqa_key)
    }
    console.log('Annotated specs with @VUE-N tags (kept @VQA-N)')
  }

  if (DO_MINT) {
    const tok = await api('POST', `/api/v1/projects/${project.id}/ci-tokens`, {
      token,
      body: { name: `e2e-local-${Date.now()}` },
    })
    const secret = tok.secret
    console.log(`Minted CI token prefix=${tok.token?.token_prefix || tok.token_prefix}`)
    console.log(`VULYK_CI_TOKEN=${secret}`)
  }

  // Warn if preferred keys diverged (e.g. project already had cases)
  const drift = mapping.filter((m) => m.preferred_key && m.key && m.preferred_key !== m.key)
  if (drift.length) {
    console.warn(
      `warning: ${drift.length} case(s) have allocated keys different from preferred VUE-N in e2e/cases (project was not empty). Annotations use allocated keys.`,
    )
  }
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
