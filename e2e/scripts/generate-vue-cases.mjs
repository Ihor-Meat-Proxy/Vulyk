#!/usr/bin/env node
/**
 * Build e2e/cases/*.json source-of-truth from existing specs + vulyk-case-map.json.
 *
 * - One JSON file per feature folder (mirrors VUE project suites).
 * - Assigns stable VUE-N keys in deterministic order (file, title).
 * - Preserves VQA key cross-links when present in the case map.
 *
 * Usage: node e2e/scripts/generate-vue-cases.mjs
 */
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const ROOT = path.resolve(__dirname, '../..')
const TESTS_DIR = path.join(ROOT, 'e2e/tests')
const CASES_DIR = path.join(ROOT, 'e2e/cases')
const MAP_PATH = path.join(ROOT, 'e2e/vulyk-case-map.json')

const TEST_RE =
  /\btest(?:\.(?:only|skip|fix|fix\.only|fix\.skip))?\(\s*(?:\/\*[\s\S]*?\*\/\s*)?(['"`])([^'"`]+)\1/g

/** Map spec filename → VUE folder name */
const FOLDER_FOR_SPEC = {
  'auth.spec.ts': 'Auth',
  'forced-password-change.spec.ts': 'Auth',
  'password-policy-lockout.spec.ts': 'Auth',
  'registration-sso.spec.ts': 'Auth',
  'setup-wizard.spec.ts': 'Auth',
  'header-menu.spec.ts': 'Shell',
  'locale-switch.spec.ts': 'Settings',
  'appearance.spec.ts': 'Settings',
  'projects-cases.spec.ts': 'Projects & cases',
  'case-list-columns.spec.ts': 'Projects & cases',
  'case-edit-keyboard.spec.ts': 'Projects & cases',
  'case-key-urls.spec.ts': 'Projects & cases',
  'case-export-pdf.spec.ts': 'Projects & cases',
  'folder-tree-lazy.spec.ts': 'Projects & cases',
  'plans-runs.spec.ts': 'Plans & runs',
  'executions.spec.ts': 'Executions',
  'result-history.spec.ts': 'Executions',
  'imports-reports.spec.ts': 'Imports & reports',
  'reports-drilldown.spec.ts': 'Imports & reports',
  'generate-automation.spec.ts': 'Automation & CI',
  'automation-repos-ci.settings.spec.ts': 'Automation & CI',
  'multi-repo-ci-ingest.spec.ts': 'Automation & CI',
  'ci-reporter-live-run.spec.ts': 'Automation & CI',
  'integrations-llm.spec.ts': 'LLM & chat',
  'llm-chat.spec.ts': 'LLM & chat',
  'chat-propose.spec.ts': 'LLM & chat',
  'admin-custom-fields.spec.ts': 'Admin',
  'admin-backups.spec.ts': 'Admin',
  'admin-updates-rollback.spec.ts': 'Admin',
  'rbac.spec.ts': 'RBAC',
  'sample-project.spec.ts': 'Sample data',
  'status-sample.spec.ts': 'Sample data',
  'pwa-mobile.spec.ts': 'PWA & a11y',
  'smoke.spec.ts': 'Smoke',
  'api-docs.spec.ts': 'Meta',
  'test-id-audit.spec.ts': 'Meta',
  // Gap-fill specs (created alongside this work)
  'settings-profile-tokens.spec.ts': 'Settings',
  'folder-tree-dnd.spec.ts': 'Projects & cases',
  'edge-cases.spec.ts': 'Edge cases',
  'rbac-roles-matrix.spec.ts': 'RBAC',
  'requirements-coverage.spec.ts': 'Requirements',
  'execution-steps.spec.ts': 'Executions',
  'error-pages.spec.ts': 'Edge cases',
  'users-admin.spec.ts': 'Admin',
}

function inferType(title) {
  const t = title.toLowerCase()
  if (
    /\b(bad|deny|denied|fail|error|invalid|weak|lockout|wrong|reject|forbidden|404|401|403|expired)\b/.test(
      t,
    )
  ) {
    return 'negative'
  }
  if (
    /\b(edge|empty|unicode|long|special|concurrent|pagination|offline|axe|keyboard|lazy)\b/.test(
      t,
    )
  ) {
    return 'edge'
  }
  return 'positive'
}

function inferPriority(type) {
  if (type === 'negative') return 'high'
  if (type === 'edge') return 'medium'
  return 'high'
}

function parseTests() {
  const tests = []
  for (const file of fs.readdirSync(TESTS_DIR).filter((f) => f.endsWith('.spec.ts'))) {
    const text = fs.readFileSync(path.join(TESTS_DIR, file), 'utf8')
    let m
    const re = new RegExp(TEST_RE.source, 'g')
    while ((m = re.exec(text))) {
      tests.push({ file, title: m[2] })
    }
  }
  tests.sort((a, b) => a.file.localeCompare(b.file) || a.title.localeCompare(b.title))
  return tests
}

function loadVqaMap() {
  if (!fs.existsSync(MAP_PATH)) return new Map()
  const data = JSON.parse(fs.readFileSync(MAP_PATH, 'utf8'))
  const map = new Map()
  for (const row of data.tests || []) {
    map.set(`e2e:${row.file} ${row.title}`, row.key)
  }
  return map
}

function main() {
  fs.mkdirSync(CASES_DIR, { recursive: true })
  // Clear previous generated feature files (keep README if any)
  for (const f of fs.readdirSync(CASES_DIR)) {
    if (f.endsWith('.json')) fs.unlinkSync(path.join(CASES_DIR, f))
  }

  const vqa = loadVqaMap()
  const tests = parseTests()
  const byFolder = new Map()
  let n = 1

  for (const t of tests) {
    const folder = FOLDER_FOR_SPEC[t.file] || 'Other'
    const external_key = `e2e:${t.file} ${t.title}`
    const type = inferType(t.title)
    const key = `VUE-${n}`
    n += 1
    const id = `${t.file.replace(/\.spec\.ts$/, '')}__${t.title}`
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '')
      .slice(0, 80)

    const caseRow = {
      id,
      key,
      title: t.title,
      type,
      priority: inferPriority(type),
      preconditions:
        'Isolated e2e stack up (./scripts/e2e-local.sh); seeded admin; VULYK_LLM_MOCK=1.',
      steps: [
        {
          action: `Run Playwright: npx playwright test ${t.file} -g "${t.title}"`,
          expected: 'Test passes; reporter posts result by VUE case key when configured.',
        },
      ],
      spec: t.file,
      test: t.title,
      external_key,
      automation_link: `e2e/tests/${t.file} :: ${t.title}`,
      vqa_key: vqa.get(external_key) || null,
      tags: ['e2e', 'playwright', `type:${type}`],
    }

    if (!byFolder.has(folder)) byFolder.set(folder, [])
    byFolder.get(folder).push(caseRow)
  }

  const manifest = {
    project: {
      key: 'VUE',
      name: 'Vulyk UI E2E',
      description:
        'Automated Playwright UI suite. Cases are synced from e2e/cases/*.json via e2e/scripts/sync-vue-project.mjs.',
      case_key_prefix: 'VUE',
    },
    folders: [],
    generated: new Date().toISOString(),
    case_count: tests.length,
  }

  for (const [folder, cases] of [...byFolder.entries()].sort((a, b) =>
    a[0].localeCompare(b[0]),
  )) {
    const slug = folder
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '')
    const file = `${slug}.json`
    fs.writeFileSync(
      path.join(CASES_DIR, file),
      JSON.stringify({ folder, cases }, null, 2) + '\n',
    )
    manifest.folders.push({ name: folder, file, count: cases.length })
  }

  fs.writeFileSync(
    path.join(CASES_DIR, 'manifest.json'),
    JSON.stringify(manifest, null, 2) + '\n',
  )
  console.log(
    `Wrote ${tests.length} cases across ${manifest.folders.length} folders → e2e/cases/`,
  )

  if (process.argv.includes('--annotate')) {
    annotateSpecs(byFolder)
  }
}

function readSpecOrNull(filePath) {
  try {
    return fs.readFileSync(filePath, 'utf8')
  } catch (err) {
    if (err && err.code === 'ENOENT') return null
    throw err
  }
}

function annotateSpecs(byFolder) {
  const TESTS = path.join(ROOT, 'e2e/tests')
  for (const cases of byFolder.values()) {
    for (const c of cases) {
      const full = path.join(TESTS, c.spec)
      // Open once (no existsSync → write TOCTOU); skip missing specs.
      let text = readSpecOrNull(full)
      if (text == null) continue
      const callRe =
        /\b(test(?:\.(?:only|skip|fix|fix\.only|fix\.skip))?)\(\s*(['"`])([^'"`]+)\2/g
      let match
      const edits = []
      while ((match = callRe.exec(text))) {
        if (match[3] !== c.test) continue
        const afterTitle = match.index + match[0].length
        const rest = text.slice(afterTitle)
        const lead = rest.match(/^\s*,\s*/)?.[0] ?? ''
        if (!lead) continue
        const optsStart = afterTitle + lead.length
        if (text[optsStart] !== '{') {
          // No options object — inject one
          const tags = c.vqa_key
            ? `['@${c.vqa_key}', '@${c.key}']`
            : `['@${c.key}']`
          const anns = c.vqa_key
            ? `[{ type: 'vulyk', description: '${c.vqa_key}' }, { type: 'vulyk', description: '${c.key}' }]`
            : `{ type: 'vulyk', description: '${c.key}' }`
          edits.push({
            start: afterTitle,
            end: afterTitle + lead.length,
            text: `, {\n  tag: ${tags}, annotation: ${anns},\n}${lead}`,
          })
          continue
        }
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
        if (body.includes(`@${c.key}`) && body.includes(`'${c.key}'`)) continue
        body = body
          .replace(/,?\s*tag\s*:\s*(?:\[[^\]]*\]|['"][^'"]+['"])/g, '')
          .replace(/,?\s*annotation\s*:\s*(?:\[[^\]]*\]|\{[^}]*\})/g, '')
          .replace(/^\s*,/, '')
          .replace(/,\s*$/, '')
          .trim()
        const tags = c.vqa_key
          ? `['@${c.vqa_key}', '@${c.key}']`
          : `['@${c.key}']`
        const anns = c.vqa_key
          ? `[{ type: 'vulyk', description: '${c.vqa_key}' }, { type: 'vulyk', description: '${c.key}' }]`
          : `{ type: 'vulyk', description: '${c.key}' }`
        const inject = `tag: ${tags}, annotation: ${anns}`
        const newBody = body ? `\n  ${inject},\n  ${body}\n` : `\n  ${inject},\n`
        edits.push({ start: optsStart, end: end + 1, text: `{${newBody}}` })
      }
      if (!edits.length) continue
      edits.sort((a, b) => b.start - a.start)
      for (const e of edits) {
        text = text.slice(0, e.start) + e.text + text.slice(e.end)
      }
      fs.writeFileSync(full, text)
    }
  }
  console.log('Annotated specs with @VUE-N (+ kept @VQA-N when known)')
}

main()
