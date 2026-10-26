import { expect, test } from '@playwright/test'
import {
  apiJson,
  createCase,
  createProject,
  createRun,
  createSuite,
} from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { ReportsPage } from '../pages/ReportsPage'
import { TestIds } from '../../frontend/src/testing/testIds'

test.describe.configure({ mode: 'serial' })

let projectId = ''
let projectKey = ''
let runId = ''
let caseKeyA = ''
let caseKeyB = ''
let caseAId = ''

test.beforeAll(async ({ request }) => {
  const admin = await ensureAdminSession(request)
  projectKey = uniqueKey('RD')
  const project = await createProject(request, admin.token, {
    key: projectKey,
    name: `Reports drill ${projectKey}`,
  })
  projectId = project.id
  const suite = await createSuite(request, admin.token, projectId, 'Auth')
  const caseA = await createCase(request, admin.token, projectId, {
    key: `${projectKey}-1`,
    title: `Login ${projectKey}`,
    suite_id: suite.id,
    priority: 'high',
    status: 'ready',
    steps: [{ action: 'Open login', expected_result: 'Form shown' }],
  })
  const caseB = await createCase(request, admin.token, projectId, {
    key: `${projectKey}-2`,
    title: `Logout ${projectKey}`,
    suite_id: suite.id,
    priority: 'medium',
    status: 'ready',
    steps: [{ action: 'Click logout', expected_result: 'Home' }],
  })
  caseKeyA = caseA.case.key
  caseKeyB = caseB.case.key
  caseAId = caseA.case.id
  const caseBId = caseB.case.id

  const prev = await createRun(request, admin.token, projectId, {
    name: `Prev ${projectKey}`,
    source: 'ci',
    include_all_cases: true,
    start: true,
  })
  const prevId = prev.run.id
  await apiJson(request, 'POST', `/api/v1/projects/${projectId}/runs/${prevId}/results`, {
    token: admin.token,
    data: {
      results: [
        { case_id: caseAId, status: 'passed', message: 'ok' },
        { case_id: caseBId, status: 'failed', message: 'boom' },
      ],
    },
  })

  const curr = await createRun(request, admin.token, projectId, {
    name: `Curr ${projectKey}`,
    source: 'ci',
    meta: {
      branch: 'main',
      commit: 'abc123def',
      pipeline_url: 'https://example.invalid/pipeline/1',
    },
    include_all_cases: true,
    start: true,
  })
  runId = curr.run.id
  await apiJson(request, 'POST', `/api/v1/projects/${projectId}/runs/${runId}/results`, {
    token: admin.token,
    data: {
      results: [
        {
          case_id: caseAId,
          status: 'failed',
          message: 'timeout',
          steps: [
            {
              position: 0,
              action: 'Open login',
              expected_result: 'Form shown',
              actual_result: 'Spinner',
              status: 'failed',
            },
          ],
        },
        { case_id: caseBId, status: 'passed', message: 'fixed' },
      ],
    },
  })
})

async function selectProject(page: import('@playwright/test').Page) {
  const select = page.locator('.project-select')
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })
}

function rowByKey(page: import('@playwright/test').Page, caseKey: string) {
  return page.locator(
    `[data-testid="${TestIds.reportsRunRow}"][data-entity-key="${caseKey}"]`,
  )
}

test('summary card opens run report with breadcrumbs', async ({ page }) => {
  await loginAsAdmin(page)
  await selectProject(page)
  const reports = new ReportsPage(page)
  await reports.open()
  await reports.expectKpis()

  await reports.openSummary(runId)
  await expect(page).toHaveURL(new RegExp(`/reports/runs/${runId}`))
  await expect(page.getByTestId(TestIds.reportsRunBreadcrumbs)).toBeVisible()
  await expect(page.getByTestId(TestIds.reportsRunHeader)).toBeVisible()
  await expect(page.getByTestId(TestIds.reportsRunSource)).toContainText(/CI|pipeline|ci/i)
})

test('failed bar segment pre-filters results', async ({ page }) => {
  await loginAsAdmin(page)
  await selectProject(page)
  const reports = new ReportsPage(page)
  await reports.open()

  const failedSeg = reports.summarySegFailed(runId)
  await expect(failedSeg).toBeVisible()
  const tag = await failedSeg.evaluate((el) => el.tagName.toLowerCase())
  if (tag === 'button') {
    await failedSeg.click()
  } else {
    await reports.summaryLocator(runId).click()
    await page.getByTestId(TestIds.reportsRunFilterStatus).selectOption('failed')
  }
  await expect(page).toHaveURL(new RegExp(`/reports/runs/${runId}`))
  await expect(page.getByTestId(TestIds.reportsRunFilterStatus)).toHaveValue('failed')
  await expect(rowByKey(page, caseKeyA)).toBeVisible()
  await expect(rowByKey(page, caseKeyB)).toHaveCount(0)
})

test('expand row shows steps; export filtered CSV; Backspace returns', async ({
  page,
}) => {
  await loginAsAdmin(page)
  await selectProject(page)
  await page.goto(`/reports/runs/${runId}`)
  await expect(page.getByTestId(TestIds.reportsRunPage)).toBeVisible()

  await page
    .locator(
      `[data-testid="${TestIds.reportsRunRowExpand}"][data-entity-id="${caseAId}"]`,
    )
    .click()
  await expect(
    page.locator(
      `[data-testid="${TestIds.reportsRunRowDetail}"][data-entity-id="${caseAId}"]`,
    ),
  ).toBeVisible()
  await expect(page.getByText(/Open login|Form shown|Spinner/i).first()).toBeVisible()

  await page.getByTestId(TestIds.reportsRunFilterStatus).selectOption('failed')
  const downloadPromise = page.waitForEvent('download')
  await page.getByTestId(TestIds.reportsRunExportCsv).click()
  const download = await downloadPromise
  expect(download.suggestedFilename()).toMatch(/\.csv$/i)

  await page.locator('body').click()
  await page.keyboard.press('Backspace')
  await expect(page).toHaveURL(/\/reports\/?$/)
})

test('KPI cards drill to cases, runs, and coverage', async ({ page }) => {
  await loginAsAdmin(page)
  await selectProject(page)
  const reports = new ReportsPage(page)
  await reports.open()

  await page.getByTestId(TestIds.reportsKpiCases).click()
  await expect(page).toHaveURL(/\/$|\/\?/)

  await reports.open()
  await page.getByTestId(TestIds.reportsKpiRuns).click()
  await expect(page).toHaveURL(/\/runs/)

  await reports.open()
  await page.getByTestId(TestIds.reportsKpiCoverage).click()
  await expect(page).toHaveURL(/\/reports\/coverage/)
  await expect(page.getByTestId(TestIds.reportsCoveragePage)).toBeVisible()
})
