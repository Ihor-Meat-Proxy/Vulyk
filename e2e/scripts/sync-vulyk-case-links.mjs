#!/usr/bin/env node
/**
 * Sync Playwright e2e tests ↔ Vulyk QA seed cases.
 *
 * - Parses e2e/tests/*.spec.ts for test() titles
 * - Ensures a seed case exists with external_key `e2e:{file} {title}`
 * - Writes/updates backend/seed/vulyk-qa.d/playwright-e2e.json (keys omitted)
 * - Patches each test() with tag + annotation `@VQA-N` after allocation
 *
 * Usage:
 *   node e2e/scripts/sync-vulyk-case-links.mjs           # write fragment + dry report
 *   node e2e/scripts/sync-vulyk-case-links.mjs --annotate # also patch specs (needs allocated keys)
 *   node e2e/scripts/sync-vulyk-case-links.mjs --check    # fail if a test lacks vulyk annotation
 */
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const ROOT = path.resolve(__dirname, '../..')
const TESTS_DIR = path.join(ROOT, 'e2e/tests')
const FRAG_PATH = path.join(ROOT, 'backend/seed/vulyk-qa.d/playwright-e2e.json')
const SAMPLE_RS = path.join(ROOT, 'backend/src/sample_seed.rs')
const BASE_SEED = path.join(ROOT, 'backend/seed/vulyk-qa.json')
const FRAG_DIR = path.join(ROOT, 'backend/seed/vulyk-qa.d')

const args = new Set(process.argv.slice(2))
const DO_ANNOTATE = args.has('--annotate')
const DO_CHECK = args.has('--check')

const TEST_RE =
  /\btest(?:\.(?:only|skip|fix|fix\.only|fix\.skip))?\(\s*(?:\/\*[\s\S]*?\*\/\s*)?(['"`])([^'"`]+)\1/g

const VULYK_ANN_RE =
  /annotation\s*:\s*\{\s*type\s*:\s*['"]vulyk['"]\s*,\s*description\s*:\s*['"](VQA-\d+)['"]\s*\}/
const VULYK_TAG_RE = /tag\s*:\s*['"]@(VQA-\d+)['"]/

function walkCases(suite, out) {
  for (const c of suite.cases || []) out.push(c)
  for (const ch of suite.children || []) walkCases(ch, out)
}

function loadAllCases() {
  const cases = []
  const base = JSON.parse(fs.readFileSync(BASE_SEED, 'utf8'))
  for (const s of base.suites || []) walkCases(s, cases)
  for (const f of fs.readdirSync(FRAG_DIR).filter((x) => x.endsWith('.json'))) {
    if (f === 'playwright-e2e.json') continue // rebuilt below
    const d = JSON.parse(fs.readFileSync(path.join(FRAG_DIR, f), 'utf8'))
    for (const s of d.suites || []) walkCases(s, cases)
  }
  return cases
}

function allocateKeys(allCasesIncludingFrag) {
  const KEY_RE = /^([A-Z][A-Z0-9]{1,7})-([0-9]+)$/
  const prefix = 'VQA'
  let maxN = 0
  const seen = new Map()
  const empties = []
  for (const c of allCasesIncludingFrag) {
    const raw = (c.key || '').trim()
    if (!raw) {
      empties.push(c)
      continue
    }
    const m = KEY_RE.exec(raw.toUpperCase())
    if (!m || m[1] !== prefix) continue
    const n = Number(m[2])
    maxN = Math.max(maxN, n)
    seen.set(`${prefix}-${n}`, c)
  }
  let next = maxN + 1
  for (const c of empties) {
    while (seen.has(`${prefix}-${next}`)) next += 1
    const key = `${prefix}-${next}`
    c.key = key
    seen.set(key, c)
    next += 1
  }
  return { maxN, allocated: empties.length }
}

function parseTests() {
  const tests = []
  for (const file of fs.readdirSync(TESTS_DIR).filter((f) => f.endsWith('.spec.ts'))) {
    const full = path.join(TESTS_DIR, file)
    const text = fs.readFileSync(full, 'utf8')
    let m
    const re = new RegExp(TEST_RE.source, 'g')
    while ((m = re.exec(text))) {
      tests.push({ file, title: m[2], index: m.index, full })
    }
  }
  return tests
}

function externalKey(file, title) {
  return `e2e:${file} ${title}`
}

function loc(en, uk = en) {
  return { en, uk, fr: en, es: en, de: en }
}

function buildCase(file, title, key) {
  return {
    ...(key ? { key } : {}),
    title: loc(title),
    description: loc(
      `Playwright e2e coverage: ${file} — ${title}`,
      `Покриття Playwright e2e: ${file} — ${title}`,
    ),
    precondition: loc(
      'API + UI running; seeded admin / fixtures as required by the spec.',
      'API + UI запущені; seed admin / fixtures за вимогами спеки.',
    ),
    priority: 'high',
    status: 'ready',
    tags: ['e2e', 'playwright', 'regression', 'type:functional', 'vulyk-qa'],
    external_key: externalKey(file, title),
    steps: [
      {
        action: loc(`Run npx playwright test ${file} -g "${title}"`),
        expected_result: loc('Test passes; Vulyk reporter posts result by case key when configured.'),
        data: loc(''),
      },
    ],
  }
}

function ensureFragmentRegistered() {
  let rs = fs.readFileSync(SAMPLE_RS, 'utf8')
  if (rs.includes('playwright-e2e.json')) return false
  const needle = 'const VULYK_QA_FRAGMENTS: &[(&str, &str)] = &['
  const idx = rs.indexOf(needle)
  if (idx < 0) throw new Error('VULYK_QA_FRAGMENTS not found')
  const insert = `
    (
        "playwright-e2e",
        include_str!("../seed/vulyk-qa.d/playwright-e2e.json"),
    ),`
  const at = idx + needle.length
  rs = rs.slice(0, at) + insert + rs.slice(at)
  fs.writeFileSync(SAMPLE_RS, rs)
  return true
}

function stripExistingVulykOptions(optionsBlock) {
  // Remove prior tag/annotation for vulyk so we can re-insert cleanly.
  return optionsBlock
    .replace(/,?\s*tag\s*:\s*['"]@VQA-\d+['"]/g, '')
    .replace(
      /,?\s*annotation\s*:\s*\{\s*type\s*:\s*['"]vulyk['"]\s*,\s*description\s*:\s*['"]VQA-\d+['"]\s*\}/g,
      '',
    )
    .replace(/^\s*,/, '')
    .replace(/,\s*$/, '')
    .trim()
}

function annotateFile(filePath, titleToKey) {
  let text = fs.readFileSync(filePath, 'utf8')
  const file = path.basename(filePath)

  // Walk every test('title'…) call and ensure options include tag + annotation.
  const callRe =
    /\b(test(?:\.(?:only|skip|fix|fix\.only|fix\.skip))?)\(\s*(['"`])([^'"`]+)\2/g
  let match
  const edits = []
  while ((match = callRe.exec(text))) {
    const title = match[3]
    const key = titleToKey.get(`${file}\0${title}`)
    if (!key) continue
    const afterTitle = match.index + match[0].length
    const rest = text.slice(afterTitle)
    const lead = rest.match(/^\s*,\s*/)?.[0] ?? ''
    if (!lead) continue
    const optsStart = afterTitle + lead.length
    const inject = `tag: '@${key}', annotation: { type: 'vulyk', description: '${key}' }`

    if (text[optsStart] === '{') {
      // Existing options object — find matching closing brace for the options only
      // (first `{` after title comma; stop at `},` before the test fn).
      let depth = 0
      let end = -1
      for (let i = optsStart; i < text.length; i++) {
        const ch = text[i]
        if (ch === '{') depth += 1
        else if (ch === '}') {
          depth -= 1
          if (depth === 0) {
            end = i
            break
          }
        }
      }
      if (end < 0) continue
      const body = text.slice(optsStart + 1, end)
      if (
        body.includes(`description: '${key}'`) ||
        body.includes(`description: "${key}"`)
      ) {
        continue // already annotated with this key
      }
      const cleaned = stripExistingVulykOptions(body)
      const newBody = cleaned ? `\n  ${inject},\n  ${cleaned.trim()}\n` : `\n  ${inject},\n`
      edits.push({ start: optsStart, end: end + 1, text: `{${newBody}}` })
    } else {
      edits.push({
        start: afterTitle,
        end: afterTitle + lead.length,
        text: `, {\n  ${inject},\n}${lead}`,
      })
    }
  }

  // Apply from end so indices stay valid
  edits.sort((a, b) => b.start - a.start)
  for (const e of edits) {
    text = text.slice(0, e.start) + e.text + text.slice(e.end)
  }
  fs.writeFileSync(filePath, text)
}

function checkAnnotations(tests) {
  const missing = []
  for (const t of tests) {
    const text = fs.readFileSync(t.full, 'utf8')
    // Find the call site and look ahead ~400 chars for annotation
    const idx = text.indexOf(`'${t.title}'`)
    const idx2 = text.indexOf(`"${t.title}"`)
    const start = idx >= 0 ? idx : idx2
    if (start < 0) {
      missing.push(`${t.file} :: ${t.title} (title not found)`)
      continue
    }
    const window = text.slice(start, start + 500)
    if (!VULYK_ANN_RE.test(window) || !VULYK_TAG_RE.test(window)) {
      missing.push(`${t.file} :: ${t.title}`)
    }
  }
  if (missing.length) {
    console.error(`error: ${missing.length} Playwright test(s) missing Vulyk case-key tag/annotation:`)
    for (const m of missing) console.error(`  - ${m}`)
    console.error(
      'Add: test(\'…\', { tag: \'@VQA-N\', annotation: { type: \'vulyk\', description: \'VQA-N\' } }, …)',
    )
    process.exit(1)
  }
  console.log(`OK: ${tests.length} Playwright tests have Vulyk case-key annotations`)
}

function main() {
  const tests = parseTests()
  if (DO_CHECK) {
    checkAnnotations(tests)
    return
  }

  const existing = loadAllCases()
  const byExt = new Map()
  for (const c of existing) {
    const ek = (c.external_key || '').trim()
    if (ek) byExt.set(ek, c)
  }

  const fragCases = []
  const mapping = [] // {file, title, key?, external_key, source}

  for (const t of tests) {
    const ek = externalKey(t.file, t.title)
    const hit = byExt.get(ek)
    if (hit && (hit.key || '').trim()) {
      mapping.push({
        file: t.file,
        title: t.title,
        key: hit.key.trim(),
        external_key: ek,
        source: 'existing',
      })
      continue
    }
    // Reuse empty-key existing with same external_key
    if (hit) {
      fragCases.push(buildCase(t.file, t.title, null))
      mapping.push({
        file: t.file,
        title: t.title,
        key: null,
        external_key: ek,
        source: 'fragment-from-existing-ext',
      })
      continue
    }
    fragCases.push(buildCase(t.file, t.title, null))
    mapping.push({
      file: t.file,
      title: t.title,
      key: null,
      external_key: ek,
      source: 'fragment-new',
    })
  }

  const fragment = {
    feature: 'playwright-e2e',
    suites: [
      {
        name: loc('Playwright e2e', 'Playwright e2e'),
        children: [],
        cases: fragCases,
      },
    ],
  }

  // Allocate across base+other frags+this fragment, then bake keys into the
  // fragment so Playwright @VQA-N tags stay stable across rebases.
  const all = [...existing, ...fragCases]
  allocateKeys(all)

  const fragByExt = new Map(fragCases.map((c) => [c.external_key, c]))
  for (const row of mapping) {
    if (row.key) continue
    const c = fragByExt.get(row.external_key)
    if (c?.key) row.key = c.key
  }

  // Prefer omitting keys in other feature PRs; this fragment owns a 1:1 e2e map
  // so keys are intentional and checked for uniqueness by check-seed-keys.sh.
  fs.writeFileSync(FRAG_PATH, JSON.stringify(fragment, null, 2) + '\n')
  const registered = ensureFragmentRegistered()

  console.log(
    `Wrote ${fragCases.length} cases → ${path.relative(ROOT, FRAG_PATH)} (registered=${registered})`,
  )
  console.log(
    `Mapping: ${mapping.filter((m) => m.source === 'existing').length} existing, ${fragCases.length} fragment`,
  )

  const titleToKey = new Map(mapping.map((m) => [`${m.file}\0${m.title}`, m.key]))
  const missingKeys = mapping.filter((m) => !m.key)
  if (missingKeys.length) {
    console.error('error: failed to allocate keys for', missingKeys.length, 'tests')
    process.exit(1)
  }

  // Persist mapping for docs / CI
  const mapPath = path.join(ROOT, 'e2e/vulyk-case-map.json')
  fs.writeFileSync(
    mapPath,
    JSON.stringify(
      {
        generated: new Date().toISOString(),
        tests: mapping.sort((a, b) =>
          a.file === b.file ? a.title.localeCompare(b.title) : a.file.localeCompare(b.file),
        ),
      },
      null,
      2,
    ) + '\n',
  )
  console.log(`Wrote ${path.relative(ROOT, mapPath)}`)

  if (DO_ANNOTATE) {
    const byFile = new Map()
    for (const t of tests) {
      if (!byFile.has(t.full)) byFile.set(t.full, titleToKey)
    }
    for (const filePath of byFile.keys()) {
      annotateFile(filePath, titleToKey)
    }
    console.log(`Annotated ${byFile.size} spec files`)
  } else {
    console.log('Dry run for annotations; re-run with --annotate to patch specs.')
  }
}

main()
