#!/usr/bin/env node
/**
 * Guard against new hard-coded English in TSX UI surfaces.
 * Scans short JSX text nodes and common user-facing attributes.
 *
 * Allowlist: frontend/scripts/i18n-literal-allowlist.txt (one regex per line)
 */
import { readFileSync, readdirSync, statSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const srcRoot = path.join(__dirname, '..', 'src')
const allowPath = path.join(__dirname, 'i18n-literal-allowlist.txt')
const ATTRS = ['title', 'placeholder', 'alt', 'aria-label', 'aria-description']

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    const p = path.join(dir, name)
    const st = statSync(p)
    if (st.isDirectory()) {
      if (name === 'i18n' || name === 'testing') continue
      walk(p, out)
    } else if (name.endsWith('.tsx') && !name.endsWith('.test.tsx')) {
      out.push(p)
    }
  }
  return out
}

function loadAllowlist() {
  return readFileSync(allowPath, 'utf8')
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith('#'))
    .map((l) => new RegExp(l))
}

const allow = loadAllowlist()
const files = walk(srcRoot)
const hits = []

// Short JSX text only (avoid matching across TS generics / multi-line code).
const jsxText = />\s*([A-Za-z][^<>{}/;:=\n]{0,80}?)\s*</g
const attrRe = new RegExp(
  `\\b(${ATTRS.join('|')})\\s*=\\s*(["'])([^"'\\n]{3,})\\2`,
  'g',
)

function isBoring(text) {
  const t = text.trim()
  if (t.length < 3 || t.length > 120) return true
  if (/^[\d\s./:_+-]+$/.test(t)) return true
  if (/^(https?:|\/|mailto:)/i.test(t)) return true
  if (/^[a-z][a-z0-9_]*$/i.test(t) && !/\s/.test(t) && t.length <= 24) return true
  // TypeScript / code fragments
  if (/\b(Promise|Record|Partial|Set|Map|boolean|string|number|null|undefined)\b/.test(t))
    return true
  if (/[()[\]]/.test(t)) return true
  if (t.includes('=>') || t.includes('?.') || t.includes('localStorage')) return true
  return false
}

function allowed(file, text) {
  const rel = path.relative(path.join(__dirname, '..'), file)
  const key = `${rel}::${text}`
  return allow.some((re) => re.test(key) || re.test(text))
}

for (const file of files) {
  const src = readFileSync(file, 'utf8')
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/^\s*\/\/.*$/gm, '')

  for (const m of src.matchAll(jsxText)) {
    const text = m[1].replace(/\s+/g, ' ').trim()
    if (isBoring(text)) continue
    if (!/[A-Za-z]{2,}/.test(text)) continue
    // Require either a space (phrase) or an initial capital (label)
    if (!/\s/.test(text) && !/^[A-Z]/.test(text)) continue
    if (!allowed(file, text)) hits.push({ file, kind: 'jsx-text', text })
  }

  for (const m of src.matchAll(attrRe)) {
    const text = m[3].replace(/\s+/g, ' ').trim()
    if (isBoring(text)) continue
    if (text.includes('{')) continue
    if (!allowed(file, text)) hits.push({ file, kind: `attr:${m[1]}`, text })
  }
}

if (hits.length) {
  console.error(`check-jsx-literals: ${hits.length} hard-coded string(s):\n`)
  for (const h of hits.slice(0, 80)) {
    const rel = path.relative(path.join(__dirname, '..'), h.file)
    console.error(`  ${rel} (${h.kind}): ${JSON.stringify(h.text)}`)
  }
  if (hits.length > 80) console.error(`  … +${hits.length - 80} more`)
  console.error(
    `\nAllowlist false positives in frontend/scripts/i18n-literal-allowlist.txt`,
  )
  process.exit(1)
}

console.log(`check-jsx-literals: ok (${files.length} files)`)
