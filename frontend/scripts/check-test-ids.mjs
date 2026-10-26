#!/usr/bin/env node
/**
 * Static audit: fail on duplicate data-testid string literals in a file,
 * interactive JSX missing test id wiring, and suspicious PII/secret literals.
 *
 * Heuristic JSX scan (handles `=>` inside attrs). See docs/testing/test-ids.md
 */
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const srcRoot = path.resolve(__dirname, '../src')

const INTERACTIVE_START =
  /<(button|input|select|textarea|a|NavLink|Link)\b/g

const HAS_TEST_ID =
  /data-testid=\{|data-testid="|testIdProps\(|listItemProps\(|listChildProps\(/

const SKIP_FILES = new Set(['vite-env.d.ts', 'main.tsx', 'App.tsx'])

/** Same testid allowed when used with listItemProps / entity attrs (list rows). */
const LIST_BASE_IDS = new Set([
  'case-row',
  'folder-node',
  'folder-case-item',
  'run-row',
  'run-detail-case-row',
  'execution-node',
  'requirement-row',
  'admin-user-row',
  'admin-project-row',
  'settings-token-row',
  'chat-message',
  'case-picker-item',
  'case-picker-folder',
  'case-picker-chip',
  'automated-suite-row',
  'case-detail-step',
  'case-detail-custom-field-input',
  'case-list-column-item',
  'header-language-item',
  'login-sso-button',
  'run-details-result-row',
  'status-badge',
  'appearance-font-size-item',
  'reports-run-summary-item',
  'reports-trend-item',
  'reports-flaky-item',
  'reports-coverage-req',
  'reports-run-group-row',
  'reports-run-row',
  'reports-run-row-expand',
  'reports-run-row-key',
  'reports-run-row-detail',
  'reports-run-comparison',
  'reports-summary-seg-passed',
  'reports-summary-seg-failed',
  'reports-summary-seg-other',
  'reports-trend-seg-passed',
  'reports-trend-seg-failed',
  'reports-trend-seg-other',
  'runs-ci-dispatch-button',
  'runs-ci-external-link',
  'import-mapping-input',
  'settings-automation-repo-row',
  'settings-automation-project-checkbox',
  'settings-ci-token-row',
  'admin-membership-row',
  'admin-token-row',
  'admin-integration-row',
  'admin-llm-row',
  'admin-sso-row',
  'chat-draft-case',
  'chat-draft-requirement',
  'iteration-flaky-item',
  'iteration-slow-item',
  'app-toast',
  'case-detail-breadcrumb-folder',
])
function walk(dir, out = []) {
  for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, ent.name)
    if (ent.isDirectory()) {
      if (
        ent.name === 'testing' ||
        ent.name === 'i18n' ||
        ent.name === 'theme' ||
        ent.name === 'pwa' ||
        ent.name === 'hooks'
      ) {
        continue
      }
      walk(p, out)
    } else if (/\.tsx$/.test(ent.name) && !SKIP_FILES.has(ent.name)) {
      out.push(p)
    }
  }
  return out
}

function stripComments(src) {
  return src
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/(^|[^:])\/\/.*$/gm, '$1')
}

/** Find end index of a JSX opening tag, ignoring `>` inside `{...}` and `=>`. */
function findOpenTagEnd(src, start) {
  let i = start
  let brace = 0
  let quote = null
  while (i < src.length) {
    const ch = src[i]
    if (quote) {
      if (ch === '\\' && quote !== '`') {
        i += 2
        continue
      }
      if (ch === quote) quote = null
      i++
      continue
    }
    if (ch === '"' || ch === "'" || ch === '`') {
      quote = ch
      i++
      continue
    }
    if (ch === '{') {
      brace++
      i++
      continue
    }
    if (ch === '}') {
      brace = Math.max(0, brace - 1)
      i++
      continue
    }
    if (ch === '>' && brace === 0) {
      if (i > 0 && src[i - 1] === '=') {
        i++
        continue
      }
      return i
    }
    i++
  }
  return -1
}

function extractLiteralTestIds(src) {
  const ids = []
  const dataRe = /data-testid=["']([a-z0-9-]+)["']/g
  let m
  while ((m = dataRe.exec(src))) ids.push({ id: m[1], via: 'attr' })
  const litProps = /(?:testIdProps|listItemProps)\(\s*['"]([a-z0-9-]+)['"]/g
  while ((m = litProps.exec(src))) ids.push({ id: m[1], via: 'helper' })
  const tmpl = /data-testid=\{`[^`]*\$\{[^`]+`\}/g
  while ((m = tmpl.exec(src))) {
    ids.push({ id: `__tmpl__${m.index}`, via: 'template', raw: m[0] })
  }
  return ids
}

function suspiciousLiteral(attr, value) {
  if (value.includes('@')) return 'contains @ (possible email / PII)'
  if (/^eyJ[A-Za-z0-9_-]+\./.test(value)) return 'looks like a JWT'
  if (/^(bearer|basic)\s/i.test(value)) {
    return 'looks like an Authorization header'
  }
  if (attr === 'data-testid') {
    if (
      /(?:^|-)[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
        value,
      )
    ) {
      return 'data-testid embeds a UUID — use data-entity-id instead'
    }
  }
  return null
}

function checkSecurityLiterals(src, rel) {
  const errors = []
  const tmplRe = /data-testid=\{`[\s\S]*?`\}/g
  let m
  while ((m = tmplRe.exec(src))) {
    if (m[0].includes('${')) {
      const line = src.slice(0, m.index).split('\n').length
      errors.push(
        `${rel}:${line}: interpolated data-testid template (use static TestIds + data-entity-id)`,
      )
    }
  }
  const emailKey = /entityKey:\s*['"][^'"]*@[^'"]*['"]/g
  while ((m = emailKey.exec(src))) {
    const line = src.slice(0, m.index).split('\n').length
    errors.push(`${rel}:${line}: entityKey literal looks like an email (PII)`)
  }
  const piiKey =
    /entityKey:\s*(?:user|tok|token|member|u)\.(?:email|display_name|password|secret|token)\b/g
  while ((m = piiKey.exec(src))) {
    const line = src.slice(0, m.index).split('\n').length
    errors.push(
      `${rel}:${line}: entityKey must not use email/display_name/password/secret fields`,
    )
  }
  for (const attr of ['data-testid', 'data-entity-key', 'data-entity-id']) {
    const re = new RegExp(`${attr}=["']([^"']+)["']`, 'g')
    while ((m = re.exec(src))) {
      const reason = suspiciousLiteral(attr, m[1])
      if (reason) {
        const line = src.slice(0, m.index).split('\n').length
        errors.push(`${rel}:${line}: ${attr}="${m[1]}": ${reason}`)
      }
    }
  }
  return errors
}

function loadCanonicalIds() {
  const file = path.join(srcRoot, 'testing/testIds.ts')
  const src = fs.readFileSync(file, 'utf8')
  const ids = new Map()
  const re = /(\w+):\s*'([a-z0-9-]+)'/g
  let m
  while ((m = re.exec(src))) {
    ids.set(m[2], m[1])
  }
  return ids
}

function checkFile(file) {
  const rel = path.relative(srcRoot, file)
  const raw = fs.readFileSync(file, 'utf8')
  const src = stripComments(raw)
  const errors = []

  errors.push(...checkSecurityLiterals(src, rel))

  const seen = new Map()
  for (const { id, via } of extractLiteralTestIds(src)) {
    if (via === 'template') {
      errors.push(`${rel}: interpolated data-testid template is forbidden`)
      continue
    }
    if (LIST_BASE_IDS.has(id)) continue
    const n = (seen.get(id) || 0) + 1
    seen.set(id, n)
    if (n > 1) {
      errors.push(`${rel}: duplicate data-testid "${id}"`)
    }
  }

  INTERACTIVE_START.lastIndex = 0
  let match
  while ((match = INTERACTIVE_START.exec(src))) {
    const tag = match[1]
    const openStart = match.index
    const end = findOpenTagEnd(src, openStart + 1)
    if (end === -1) continue
    const full = src.slice(openStart, end + 1)

    if (/\btype=["']hidden["']/.test(full)) continue
    if (HAS_TEST_ID.test(full)) continue

    const line = src.slice(0, openStart).split('\n').length
    errors.push(
      `${rel}:${line}: <${tag}> appears to lack data-testid / testIdProps / listItemProps`,
    )
  }

  return errors
}

function main() {
  const canonical = loadCanonicalIds()
  const valueToKeys = new Map()
  for (const [id, key] of canonical) {
    if (!valueToKeys.has(id)) valueToKeys.set(id, [])
    valueToKeys.get(id).push(key)
  }
  const moduleDupes = []
  for (const [id, keys] of valueToKeys) {
    if (keys.length > 1) {
      moduleDupes.push(
        `testing/testIds.ts: duplicate TestIds value "${id}" on keys ${keys.join(', ')}`,
      )
    }
  }

  const files = walk(srcRoot)
  const errors = [...moduleDupes]
  for (const f of files) {
    errors.push(...checkFile(f))
  }

  if (errors.length) {
    console.error(`check-test-ids: ${errors.length} issue(s)\n`)
    for (const e of errors) console.error(`  ${e}`)
    process.exit(1)
  }
  console.log(
    `check-test-ids: ok (${files.length} files, ${canonical.size} canonical ids)`,
  )
}

main()
