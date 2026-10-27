import { expect, test } from '@playwright/test'
import {
  createCase,
  createProject,
  createRun,
  createSuite,
} from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { API_URL, uniqueKey } from '../fixtures/env'
import { TestIds } from '../../frontend/src/testing/testIds'
import { CasesPage } from '../pages/CasesPage'

test.describe.configure({ mode: 'serial' })

let projectId = ''
let projectKey = ''
let caseKey = ''
let caseTitle = ''

test.beforeAll(async ({ request }) => {
  const admin = await ensureAdminSession(request)
  projectKey = uniqueKey('XP')
  const project = await createProject(request, admin.token, {
    key: projectKey,
    name: `Export ${projectKey}`,
  })
  projectId = project.id
  await createSuite(request, admin.token, projectId, 'ExportSuite')
  caseTitle = `PDF export ${projectKey}`
  const created = await createCase(request, admin.token, projectId, {
    title: caseTitle,
    priority: 'high',
    status: 'ready',
    steps: [
      {
        action: 'Click Export as PDF',
        expected_result: 'PDF downloads with case key',
      },
    ],
  })
  caseKey = created.case.key
  const run = await createRun(request, admin.token, projectId, {
    name: `Manual ${projectKey}`,
    start: true,
    case_ids: [created.case.id],
  })
  // createRun typing is run-only; load items for execute.
  const runFull = await request.get(
    `${API_URL}/api/v1/projects/${projectId}/runs/${run.run.id}`,
  )
  expect(runFull.ok()).toBeTruthy()
  const runBody = (await runFull.json()) as {
    items?: Array<{ item?: { id: string }; id?: string }>
  }
  const itemId =
    runBody.items?.[0]?.item?.id ?? runBody.items?.[0]?.id ?? null
  if (itemId) {
    await request.post(
      `${API_URL}/api/v1/projects/${projectId}/runs/${run.run.id}/results/execute`,
      {
        headers: { Authorization: `Bearer ${admin.token}` },
        data: {
          run_item_id: itemId,
          status: 'Passed',
          message: 'e2e manual',
        },
      },
    )
  }
})

test('case detail Export as PDF downloads a PDF', {
  tag: '@VQA-244', annotation: { type: 'vulyk', description: 'VQA-244' },
}, async ({ page }) => {
  await loginAsAdmin(page)
  const select = page.locator('.project-select')
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })

  const cases = new CasesPage(page)
  await cases.open()
  await cases.openCaseByTitle(caseTitle)
  await expect(cases.exportDetailButton()).toBeVisible()

  const download = await cases.exportDetailPdf()
  expect(download.suggestedFilename()).toMatch(new RegExp(`${caseKey}\\.pdf$`, 'i'))
  const path = await download.path()
  expect(path).toBeTruthy()
  const fs = await import('node:fs')
  const buf = fs.readFileSync(path!)
  expect(buf.subarray(0, 4).toString('utf8')).toBe('%PDF')
})

test('cases list row and bulk export download PDF', {
  tag: '@VQA-245', annotation: { type: 'vulyk', description: 'VQA-245' },
}, async ({ page }) => {
  await loginAsAdmin(page)
  const select = page.locator('.project-select')
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })

  const cases = new CasesPage(page)
  await cases.open()
  await expect(
    page.locator(
      `[data-testid="${TestIds.caseRowExportPdf}"][data-entity-key="${caseKey}"]`,
    ),
  ).toBeVisible({
    timeout: 30_000,
  })

  const rowDownload = await cases.exportRowPdf(caseKey)
  expect(rowDownload.suggestedFilename()).toMatch(/\.pdf$/i)

  await cases.selectCase(caseKey)
  await expect(page.getByTestId(TestIds.caseListExportPdf)).toBeVisible()
  const bulkDownload = await cases.exportSelectedPdf()
  expect(bulkDownload.suggestedFilename()).toMatch(/\.pdf$/i)
})
