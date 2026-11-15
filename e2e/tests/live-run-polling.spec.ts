import { expect, test } from '@playwright/test'
import {
  apiJson,
  createCase,
  createProject,
  createRun,
} from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { TestIds } from '../../frontend/src/testing/testIds'

test.describe.configure({ mode: 'serial' })

let projectKey = ''
let projectId = ''
let adminToken = ''
let caseKey = ''
let caseId = ''

test.beforeAll(async ({ request }) => {
  const admin = await ensureAdminSession(request)
  adminToken = admin.token
  projectKey = uniqueKey('LP')
  const project = await createProject(request, admin.token, {
    key: projectKey,
    name: `LivePoll ${projectKey}`,
  })
  projectId = project.id
  const created = await createCase(request, admin.token, projectId, {
    title: `Live poll case ${projectKey}`,
    status: 'ready',
    steps: [{ action: 'Do it', expected_result: 'ok' }],
  })
  caseId = created.case.id
  caseKey = created.case.key
})

async function selectProject(page: import('@playwright/test').Page) {
  await loginAsAdmin(page)
  const select = page.locator('.project-select')
  await expect
    .poll(async () => select.locator('option').count())
    .toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })
}

test('in_progress run detail polls progress without full reload', {
  tag: ['@VQA-356', '@VUE-218'],
  annotation: [
    { type: 'vulyk', description: 'VQA-356' },
    { type: 'vulyk', description: 'VUE-218' },
  ],
}, async ({ page, request }) => {
  const runName = `Live poll ${projectKey}`
  const created = await createRun(request, adminToken, projectId, {
    name: runName,
    start: true,
    include_all_cases: true,
  })
  const runId = created.run.id
  expect(created.run.status).toMatch(/in_progress|running/i)

  const detail = await apiJson<{
    progress: { completed: number; total: number }
    items: Array<{ item: { id: string; status: string }; case_key: string }>
  }>(request, 'GET', `/api/v1/projects/${projectId}/runs/${runId}`, {
    token: adminToken,
  })
  expect(detail.progress.total).toBeGreaterThanOrEqual(1)
  expect(detail.progress.completed).toBe(0)
  const item =
    detail.items.find((i) => i.case_key === caseKey) ?? detail.items[0]
  expect(item).toBeTruthy()

  await selectProject(page)
  await page.goto(`/runs/${runId}`)
  await expect(page.getByTestId(TestIds.runDetailPanel)).toBeVisible({
    timeout: 30_000,
  })
  await expect(page.getByTestId(TestIds.runDetailLiveBadge)).toBeVisible()
  await expect(page.getByTestId(TestIds.runDetailProgress)).toContainText(
    /0\/\d+/i,
  )

  await apiJson(
    request,
    'POST',
    `/api/v1/projects/${projectId}/runs/${runId}/results/execute`,
    {
      token: adminToken,
      data: {
        run_item_id: item.item.id,
        case_id: caseId,
        status: 'Passed',
        message: 'polled-in',
        steps: [],
      },
    },
  )

  // Default poll is ~2s; wait without reloading the page.
  await expect(page.getByTestId(TestIds.runDetailProgress)).toContainText(
    /1\/\d+/i,
    { timeout: 8_000 },
  )
  await expect(
    page
      .getByTestId(TestIds.runDetailCases)
      .locator(
        `[data-testid="${TestIds.runDetailCaseRow}"][data-entity-key="${caseKey}"]`,
      ),
  ).toBeVisible()
  await expect(page.getByTestId(TestIds.runDetailPanel)).toBeVisible()
})

test('runs list polls live status badges', {
  tag: ['@VQA-357', '@VUE-219'],
  annotation: [
    { type: 'vulyk', description: 'VQA-357' },
    { type: 'vulyk', description: 'VUE-219' },
  ],
}, async ({ page, request }) => {
  const runName = `List live ${projectKey}`
  const created = await createRun(request, adminToken, projectId, {
    name: runName,
    start: true,
    include_all_cases: true,
  })
  const runId = created.run.id

  await selectProject(page)
  await page.goto('/runs')
  await expect(page.getByTestId(TestIds.runsPage)).toBeVisible({
    timeout: 30_000,
  })
  await expect(page.getByTestId(TestIds.runsLiveBadge)).toBeVisible({
    timeout: 15_000,
  })

  const runRow = page.locator(
    `[data-testid="${TestIds.runRow}"][data-entity-id="${runId}"]`,
  )
  await expect(runRow).toBeVisible({ timeout: 15_000 })
  const badge = runRow.locator('[data-testid="runs-page-status-badge"]')
  await expect(badge).toHaveAttribute('data-status', /in_progress|running/i)

  await apiJson(request, 'PUT', `/api/v1/projects/${projectId}/runs/${runId}`, {
    token: adminToken,
    data: { status: 'closed' },
  })

  await expect(badge).toHaveAttribute('data-status', /closed|completed/i, {
    timeout: 8_000,
  })
})
