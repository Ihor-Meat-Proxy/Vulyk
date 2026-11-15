#!/usr/bin/env node
/**
 * Sync Playwright e2e tests ↔ Vulyk QA seed cases.
 *
 * - Parses e2e/tests/*.spec.ts for test() titles
 * - Ensures a seed case exists with external_key `e2e:{file} {title}`
 * - Writes/updates backend/seed/vulyk-qa.d/playwright-e2e.json
 * - Patches each test() with tag + annotation `@VQA-N` after allocation
 *
 * SAFE DEFAULT (additive):
 *   Preserves existing playwright-e2e.json keys and cases. Only appends
 *   cases for new tests (keys omitted → allocated after current max).
 *   Does NOT renumber or drop existing keys / meta cases.
 *
 * NEVER run --force-renumber on a parallel feature branch. Prefer appending
 * keys manually (see AGENTS.md / e2e/README.md).
 *
 * Usage:
 *   node e2e/scripts/sync-vulyk-case-links.mjs                 # additive write + dry report
 *   node e2e/scripts/sync-vulyk-case-links.mjs --annotate      # also patch specs missing tags
 *   node e2e/scripts/sync-vulyk-case-links.mjs --check         # fail if a test lacks annotation
 *   node e2e/scripts/sync-vulyk-case-links.mjs --force-renumber  # FULL rebuild (dangerous)
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
const FORCE_RENUMBER = args.has('--force-renumber')

const TEST_RE =
  /\btest(?:\.(?:only|skip|fix|fix\.only|fix\.skip))?\(\s*(?:\/\*[\s\S]*?\*\/\s*)?(['"`])([^'"`]+)\1/g

// Accept VQA (sample seed) and/or VUE (dedicated UI e2e project) keys.
const VULYK_ANN_RE =
  /description\s*:\s*['"]((?:VQA|VUE)-\d+)['"]/
const VULYK_TAG_RE = /@((?:VQA|VUE)-\d+)/

function walkCases(suite, out) {
  for (const c of suite.cases || []) out.push(c)
  for (const ch of suite.children || []) walkCases(ch, out)
}

function loadAllCases({ includePlaywrightFrag = false } = {}) {
  const cases = []
  const base = JSON.parse(fs.readFileSync(BASE_SEED, 'utf8'))
  for (const s of base.suites || []) walkCases(s, cases)
  for (const f of fs.readdirSync(FRAG_DIR).filter((x) => x.endsWith('.json'))) {
    if (!includePlaywrightFrag && f === 'playwright-e2e.json') continue
    const d = JSON.parse(fs.readFileSync(path.join(FRAG_DIR, f), 'utf8'))
    for (const s of d.suites || []) walkCases(s, cases)
  }
  return cases
}

function loadPlaywrightFragCases() {
  if (!fs.existsSync(FRAG_PATH)) return []
  const d = JSON.parse(fs.readFileSync(FRAG_PATH, 'utf8'))
  const cases = []
  for (const s of d.suites || []) walkCases(s, cases)
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
        body.includes(`description: "${key}"`) ||
        body.includes(`@${key}`)
      ) {
        continue // already annotated with this key
      }
      // Additive annotate: keep existing VQA tags unless --force-renumber.
      if (
        !replaceExisting &&
        (/@VQA-\d+/.test(body) || /description\s*:\s*['"]VQA-\d+['"]/.test(body))
      ) {
        continue
      }
      const cleaned = replaceExisting ? stripExistingVulykOptions(body) : body
      const newBody = cleaned.trim()
        ? `\n  ${inject},\n  ${cleaned.trim()}\n`
        : `\n  ${inject},\n`
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
  if (edits.length) fs.writeFileSync(filePath, text)
  return edits.length
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
      'Add: test(\'…\', { tag: [\'@VQA-N\', \'@VUE-N\'], annotation: [{ type: \'vulyk\', description: \'VQA-N\' }, { type: \'vulyk\', description: \'VUE-N\' }] }, …)',
    )
    process.exit(1)
  }
  console.log(`OK: ${tests.length} Playwright tests have Vulyk case-key annotations`)
}

function writeFragment(cases) {
  const fragment = {
    feature: 'playwright-e2e',
    suites: [
      {
        name: loc('Playwright e2e', 'Playwright e2e'),
        children: [],
        cases,
      },
    ],
  }
  fs.writeFileSync(FRAG_PATH, JSON.stringify(fragment, null, 2) + '\n')
}

function writeMap(mapping) {
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
}

function syncForceRenumber(tests) {
  console.warn(
    'WARNING: --force-renumber rebuilds playwright-e2e.json and may change keys. '
      + 'Do not use on parallel feature branches.',
  )
  const existing = loadAllCases({ includePlaywrightFrag: false })
  const byExt = new Map()
  for (const c of existing) {
    const ek = (c.external_key || '').trim()
    if (ek) byExt.set(ek, c)
  }

  const fragCases = []
  const mapping = []

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
    fragCases.push(buildCase(t.file, t.title, null))
    mapping.push({
      file: t.file,
      title: t.title,
      key: null,
      external_key: ek,
      source: hit ? 'fragment-from-existing-ext' : 'fragment-new',
    })
  }

  const all = [...existing, ...fragCases]
  allocateKeys(all)

  const fragByExt = new Map(fragCases.map((c) => [c.external_key, c]))
  for (const row of mapping) {
    if (row.key) continue
    const c = fragByExt.get(row.external_key)
    if (c?.key) row.key = c.key
  }

  writeFragment(fragCases)
  const registered = ensureFragmentRegistered()
  console.log(
    `Wrote ${fragCases.length} cases → ${path.relative(ROOT, FRAG_PATH)} (registered=${registered}, force-renumber)`,
  )
  return mapping
}

function syncAdditive(tests) {
  const existingFrag = loadPlaywrightFragCases()
  const byExt = new Map()
  for (const c of existingFrag) {
    const ek = (c.external_key || '').trim()
    if (ek) byExt.set(ek, c)
  }

  const others = loadAllCases({ includePlaywrightFrag: false })
  const othersByExt = new Map()
  for (const c of others) {
    const ek = (c.external_key || '').trim()
    if (ek) othersByExt.set(ek, c)
  }

  const kept = [...existingFrag]
  const mapping = []
  const newCases = []
  let reused = 0

  for (const t of tests) {
    const ek = externalKey(t.file, t.title)
    const inFrag = byExt.get(ek)
    if (inFrag && (inFrag.key || '').trim()) {
      reused += 1
      mapping.push({
        file: t.file,
        title: t.title,
        key: inFrag.key.trim(),
        external_key: ek,
        source: 'playwright-e2e',
      })
      continue
    }
    const elsewhere = othersByExt.get(ek)
    if (elsewhere && (elsewhere.key || '').trim()) {
      mapping.push({
        file: t.file,
        title: t.title,
        key: elsewhere.key.trim(),
        external_key: ek,
        source: 'existing-other-seed',
      })
      continue
    }
    if (inFrag) {
      mapping.push({
        file: t.file,
        title: t.title,
        key: null,
        external_key: ek,
        source: 'fragment-empty-key',
      })
      continue
    }
    const created = buildCase(t.file, t.title, null)
    newCases.push(created)
    kept.push(created)
    byExt.set(ek, created)
    mapping.push({
      file: t.file,
      title: t.title,
      key: null,
      external_key: ek,
      source: 'fragment-new',
    })
  }

  const testExt = new Set(tests.map((t) => externalKey(t.file, t.title)))
  const preservedMeta = existingFrag.filter((c) => {
    const ek = (c.external_key || '').trim()
    return ek && !testExt.has(ek) && !ek.startsWith('e2e:')
  }).length

  // Allocate empty keys inside the playwright fragment only.
  // Start after max(playwright-e2e) and skip numbers already claimed by any
  // seed file so we fill uncovered holes (e.g. 365..367) instead of jumping
  // to global max+1 (which makes check-e2e-case-keys.sh fail on PE gaps).
  const KEY_RE = /^([A-Z][A-Z0-9]{1,7})-([0-9]+)$/
  const claimed = new Set()
  let peMax = 0
  for (const c of [...others, ...kept]) {
    const raw = (c.key || '').trim()
    if (!raw) continue
    const m = KEY_RE.exec(raw.toUpperCase())
    if (!m || m[1] !== 'VQA') continue
    const n = Number(m[2])
    claimed.add(n)
  }
  for (const c of kept) {
    const raw = (c.key || '').trim()
    if (!raw) continue
    const m = KEY_RE.exec(raw.toUpperCase())
    if (!m || m[1] !== 'VQA') continue
    peMax = Math.max(peMax, Number(m[2]))
  }
  let next = peMax + 1
  let allocated = 0
  for (const c of kept) {
    if ((c.key || '').trim()) continue
    while (claimed.has(next)) next += 1
    c.key = `VQA-${next}`
    claimed.add(next)
    next += 1
    allocated += 1
  }

  for (const row of mapping) {
    if (row.key) continue
    const c = byExt.get(row.external_key)
    if (c?.key) row.key = c.key
  }

  writeFragment(kept)
  const registered = ensureFragmentRegistered()
  console.log(
    `Additive sync → ${path.relative(ROOT, FRAG_PATH)}: `
      + `${kept.length} cases (reused=${reused}, new=${newCases.length}, `
      + `allocated=${allocated}, preserved_meta=${preservedMeta}, registered=${registered})`,
  )
  console.log(
    'Existing keys left unchanged. Pass --force-renumber to rebuild from specs '
      + '(do not use on parallel feature branches).',
  )
  return mapping
}

function main() {
  const tests = parseTests()
  if (DO_CHECK) {
    checkAnnotations(tests)
    return
  }

  const mapping = FORCE_RENUMBER ? syncForceRenumber(tests) : syncAdditive(tests)

  const missingKeys = mapping.filter((m) => !m.key)
  if (missingKeys.length) {
    console.error('error: failed to allocate keys for', missingKeys.length, 'tests')
    process.exit(1)
  }

  writeMap(mapping)

  if (DO_ANNOTATE) {
    const titleToKey = new Map(mapping.map((m) => [`${m.file}\0${m.title}`, m.key]))
    const byFile = new Map()
    for (const t of tests) {
      if (!byFile.has(t.full)) byFile.set(t.full, titleToKey)
    }
    let edited = 0
    for (const filePath of byFile.keys()) {
      edited += annotateFile(filePath, titleToKey, {
        replaceExisting: FORCE_RENUMBER,
      })
    }
    console.log(
      FORCE_RENUMBER
        ? `Annotated ${byFile.size} spec files (force replace)`
        : `Annotated specs as needed (${edited} test option block(s) changed; existing VQA tags kept)`,
    )
  } else {
    console.log('Dry run for annotations; re-run with --annotate to patch specs.')
  }
}


main()
