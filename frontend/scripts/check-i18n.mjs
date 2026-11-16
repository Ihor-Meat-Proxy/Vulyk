#!/usr/bin/env node
/**
 * i18n completeness gate:
 * - every EN key exists in uk/fr/es/de (and vice versa)
 * - `{placeholder}` names match EN per key
 * - uk has no unexpected Latin tokens (allowlist)
 * - non-EN values identical to EN only when allowlisted
 * - JSX literals not wrapped in t() (delegates to check-jsx-literals.mjs)
 *
 * Locales: frontend/src/i18n/locales/*.ts
 * Allowlists: identical-allowlist.ts, latin-allowlist.ts
 */
import { readFileSync, readdirSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(__dirname, '..')
const localesDir = path.join(root, 'src/i18n/locales')
const i18nDir = path.join(root, 'src/i18n')

const LOCALES = ['en', 'uk', 'fr', 'es', 'de']

function parseLocaleDict(code) {
  const src = readFileSync(path.join(localesDir, `${code}.ts`), 'utf8')
  const dict = new Map()
  // 'key': "value" | 'key': 'value' | "key": "value"
  const re =
    /['"]([^'"]+)['"]\s*:\s*(`(?:\\`|[^`])*`|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')/g
  let m
  while ((m = re.exec(src))) {
    const key = m[1]
    let raw = m[2]
    let value
    if (raw.startsWith('`')) {
      value = raw.slice(1, -1).replace(/\\`/g, '`')
    } else {
      const quote = raw[0]
      value = raw.slice(1, -1).replace(new RegExp(`\\\\${quote}`, 'g'), quote)
      value = value.replace(/\\n/g, '\n').replace(/\\t/g, '\t')
    }
    dict.set(key, value)
  }
  if (dict.size < 50) {
    throw new Error(`${code}: parsed only ${dict.size} keys (parser bug?)`)
  }
  return dict
}

function parseStringSet(file, exportName) {
  const src = readFileSync(path.join(i18nDir, file), 'utf8')
  const start = src.indexOf(`export const ${exportName}`)
  if (start < 0) throw new Error(`missing export ${exportName} in ${file}`)
  const slice = src.slice(start)
  const items = []
  const re = /'((?:\\'|[^'])*)'|"((?:\\"|[^"])*)"/g
  let m
  // Stop at closing of the Set constructor array roughly by taking until `)` after `[`
  const arrayStart = slice.indexOf('[')
  const arrayEnd = slice.indexOf(']', arrayStart)
  const body = slice.slice(arrayStart, arrayEnd + 1)
  while ((m = re.exec(body))) {
    items.push((m[1] ?? m[2]).replace(/\\'/g, "'").replace(/\\"/g, '"'))
  }
  return new Set(items)
}

function placeholders(value) {
  const found = []
  const re = /\{([a-zA-Z_][a-zA-Z0-9_]*)\}/g
  let m
  while ((m = re.exec(value))) found.push(m[1])
  return [...new Set(found)].sort()
}

function latinTokensInValue(value) {
  const stripped = value.replace(/\{[^}]+\}/g, ' ')
  const raw = stripped.match(
    /[A-Za-z][A-Za-z0-9+._/-]*(?::[A-Za-z][A-Za-z0-9+._/-]*)*/g,
  )
  if (!raw) return []
  return raw.map((t) => t.replace(/[-.]+$/g, '').toLowerCase()).filter(Boolean)
}

function main() {
  const errors = []
  const dicts = Object.fromEntries(LOCALES.map((c) => [c, parseLocaleDict(c)]))
  const en = dicts.en
  const enKeys = [...en.keys()].sort()

  const identicalAllow = parseStringSet(
    'identical-allowlist.ts',
    'IDENTICAL_TO_EN_ALLOWLIST',
  )
  const latinAllow = new Set(
    [...parseStringSet('latin-allowlist.ts', 'LATIN_IN_CYRILLIC_ALLOWLIST')].map(
      (t) => t.toLowerCase(),
    ),
  )

  for (const key of identicalAllow) {
    if (!en.has(key)) {
      errors.push(`identical-allowlist unknown key: ${key}`)
    }
  }

  for (const code of LOCALES) {
    const keys = [...dicts[code].keys()].sort()
    const missing = enKeys.filter((k) => !dicts[code].has(k))
    const extra = keys.filter((k) => !en.has(k))
    if (missing.length || extra.length) {
      errors.push(
        `${code}: key parity failed (missing ${missing.length}, extra ${extra.length})`,
      )
      for (const k of missing.slice(0, 20)) errors.push(`  missing: ${k}`)
      for (const k of extra.slice(0, 20)) errors.push(`  extra: ${k}`)
    }

    const empty = [...dicts[code].entries()]
      .filter(([, v]) => !String(v).trim())
      .map(([k]) => k)
    for (const k of empty) errors.push(`${code}: empty value for ${k}`)
  }

  for (const code of LOCALES.filter((c) => c !== 'en')) {
    const dict = dicts[code]
    for (const key of enKeys) {
      if (!dict.has(key)) continue
      const enPh = placeholders(en.get(key)).join(',')
      const locPh = placeholders(dict.get(key)).join(',')
      if (enPh !== locPh) {
        errors.push(
          `${code}:${key}: placeholder mismatch en=[${enPh}] ${code}=[${locPh}]`,
        )
      }
      if (dict.get(key) === en.get(key) && !identicalAllow.has(key)) {
        errors.push(`${code}:${key}: identical to English (not allowlisted)`)
      }
    }
  }

  const uk = dicts.uk
  for (const key of enKeys) {
    if (!uk.has(key)) continue
    const bad = latinTokensInValue(String(uk.get(key))).filter(
      (t) => !latinAllow.has(t),
    )
    if (bad.length) {
      errors.push(
        `uk:${key}: unexpected Latin token(s): ${[...new Set(bad)].join(', ')}`,
      )
    }
  }

  if (errors.length) {
    console.error(`check-i18n: ${errors.length} issue(s)\n`)
    for (const e of errors.slice(0, 200)) console.error(`  ${e}`)
    if (errors.length > 200) {
      console.error(`  …and ${errors.length - 200} more`)
    }
    process.exit(1)
  }

  console.log(
    `check-i18n: ok (${enKeys.length} keys × ${LOCALES.length} locales, placeholders + uk Latin + identical allowlist)`,
  )

  // Hardcoded UI strings not in t()
  const lit = spawnSync(process.execPath, [path.join(__dirname, 'check-jsx-literals.mjs')], {
    cwd: root,
    stdio: 'inherit',
  })
  if (lit.status !== 0) process.exit(lit.status ?? 1)
}

main()
