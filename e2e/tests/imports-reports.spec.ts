import { expect, test } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { TestIds } from '../../frontend/src/testing/testIds'
import {
  createApiToken,
  createCase,
  createProject,
  createRun,
  createSuite,
} from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { API_URL, uniqueKey } from '../fixtures/env'
import { ReportsPage } from '../pages/ReportsPage'
import { RunsPage } from '../pages/RunsPage'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const fixtures = path.join(__dirname, '..', 'fixtures')

test.describe.configure({ mode: 'serial' })

let projectId = ''
let projectKey = ''
let adminToken = ''
let runId = ''
let runName = ''

test.beforeAll(async ({ request }) => {
  const admin = await ensureAdminSession(request)
  adminToken = admin.token
  projectKey = uniqueKey('IR')
  const project = await createProject(request, admin.token, {
    key: projectKey,
    name: `Import ${projectKey}`,
  })
  projectId = project.id
  await createSuite(request, admin.token, projectId, 'ImportSuite')
  await createCase(request, admin.token, projectId, {
    key: `${projectKey}-1`,
    title: `Seed ${projectKey}`,
    priority: 'medium',
    status: 'ready',
    steps: [{ action: 'x', expected_result: 'y' }],
  })
  runName = `JUnit ${projectKey}`
  const run = await createRun(request, admin.token, projectId, {
    name: runName,
    start: true,
    include_all_cases: true,
  })
  runId = run.run.id
})

test('JUnit upload via API token', {
  tag: '@VQA-260', annotation: { type: 'vulyk', description: 'VQA-260' },
}, async ({ request }) => {
  const token = await createApiToken(
    request,
    adminToken,
    `e2e-junit-${projectKey}`,
  )
  const xml = fs.readFileSync(path.join(fixtures, 'sample-junit.xml'), 'utf8')
  const res = await request.post(`${API_URL}/api/v1/runs/${runId}/ingest`, {
    headers: {
      Authorization: `Bearer ${token.secret}`,
      'Content-Type': 'application/json',
    },
    data: { format: 'junit', content: xml, source: 'e2e-api-token' },
  })
  expect(res.ok(), await res.text()).toBeTruthy()
  const body = await res.json()
  expect(body.parsed).toBeGreaterThan(0)
})

test('import TestLink, Xray, Squash TM samples', {
  tag: '@VQA-261', annotation: { type: 'vulyk', description: 'VQA-261' },
}, async ({ page }) => {
  await loginAsAdmin(page)
  const select = page.locator('.project-select')
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })

  const nav = page.getByTestId(TestIds.primaryNav)
  await nav.getByRole('link', { name: /Import|Імпорт/i }).click()
  await expect(page.getByRole('heading', { name: 'Import cases' })).toBeVisible()

  async function importWith(
    importerLabel: string,
    format: string,
    content: string,
  ) {
    await page.getByLabel('Importer').selectOption({ label: importerLabel })
    await page.getByLabel('Format').selectOption(format)
    await page.getByLabel('Source content').fill(content)
    await page.getByRole('button', { name: 'Import', exact: true }).click()
    await expect(page.getByText(/Created \d+ cases/i)).toBeVisible({
      timeout: 30_000,
    })
  }

  await importWith(
    'TestLink',
    'xml',
    fs.readFileSync(path.join(fixtures, 'testlink-sample.xml'), 'utf8'),
  )
  await importWith(
    'Jira Xray',
    'json',
    fs.readFileSync(path.join(fixtures, 'xray-sample.json'), 'utf8'),
  )
  await importWith(
    'Squash TM',
    'xml',
    fs.readFileSync(path.join(fixtures, 'squash-sample.xml'), 'utf8'),
  )
})

test('requirements can be created', {
  tag: '@VQA-262', annotation: { type: 'vulyk', description: 'VQA-262' },
}, async ({ page }) => {
  await loginAsAdmin(page)
  const select = page.locator('.project-select')
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })

  await page
    .getByTestId(TestIds.primaryNav)
    .getByRole('link', { name: /Requirements|Вимоги/i })
    .click()
  await expect(page.getByRole('heading', { name: 'Requirements' })).toBeVisible()
  const title = `Req ${projectKey}`
  await page.getByLabel('Requirement title').fill(title)
  await page.getByRole('button', { name: 'Add' }).click()
  await expect(page.getByText(title)).toBeVisible({ timeout: 20_000 })
})

test('reports page and CSV export', {
  tag: '@VQA-263', annotation: { type: 'vulyk', description: 'VQA-263' },
}, async ({ page }) => {
  await loginAsAdmin(page)
  const select = page.getByTestId(TestIds.headerProjectSelect)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })

  // Also upload via UI path once for smoke continuity
  const runs = new RunsPage(page)
  await runs.uploadJunit(runName, path.join(fixtures, 'sample-junit.xml')).catch(
    () => undefined,
  )

  const reports = new ReportsPage(page)
  await reports.open()
  await reports.expectKpis()
  await reports.exportCsv()
})
