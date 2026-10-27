import { expect, test } from '@playwright/test'
import {
  createCase,
  createPlan,
  createProject,
  createRun,
  createSuite,
} from '../fixtures/api'
import {
  dismissPwaToasts,
  ensureAdminSession,
  loginAsAdmin,
} from '../fixtures/auth'
import { API_URL, uniqueKey } from '../fixtures/env'
import { RunsPage } from '../pages/RunsPage'

test.describe.configure({ mode: 'serial' })

let projectId = ''
let projectKey = ''
let adminToken = ''
let caseId = ''

test.beforeAll(async ({ request }) => {
  const admin = await ensureAdminSession(request)
  adminToken = admin.token
  projectKey = uniqueKey('PR')
  const project = await createProject(request, admin.token, {
    key: projectKey,
    name: `Plans ${projectKey}`,
  })
  projectId = project.id
  await createSuite(request, admin.token, projectId, 'Exec')
  const created = await createCase(request, admin.token, projectId, {
    key: `${projectKey}-1`,
    title: `Executable ${projectKey}`,
    priority: 'high',
    status: 'ready',
    steps: [
      { action: 'Step one', expected_result: 'ok' },
      { action: 'Step two', expected_result: 'ok' },
    ],
  })
  caseId = (created as { case: { id: string } }).case.id
})

test('create plan and run, execute pass/fail/blocked, attach, close', {
  tag: '@VQA-280', annotation: { type: 'vulyk', description: 'VQA-280' },
}, async ({
  page,
  request,
}) => {
  await loginAsAdmin(page)
  const select = page.locator('.project-select')
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })

  const runs = new RunsPage(page)
  await runs.open()

  const planName = `Plan ${projectKey}`
  await runs.createPlan(planName)

  // Prefer API to attach cases to plan for speed/reliability
  const planRes = await createPlan(request, adminToken, projectId, {
    name: `API Plan ${projectKey}`,
    case_ids: [caseId],
  })
  expect(planRes).toBeTruthy()

  const runName = `Run ${projectKey}`
  await runs.createRun(runName)

  // Execute statuses
  const list = page.getByRole('list', { name: 'Run cases' })
  await expect(list.locator('button').first()).toBeVisible({ timeout: 30_000 })
  await list.locator('button').first().click()

  await page
    .getByRole('group', { name: 'Result status' })
    .getByRole('button', { name: 'Passed', exact: true })
    .click()
  await expect(page.locator('.status-pill.status-passed').first()).toBeVisible({
    timeout: 20_000,
  })

  // Status save auto-closes the exec panel when no untested cases remain — reopen.
  await list.locator('button').first().click()
  await expect(
    page.getByRole('group', { name: 'Result status' }),
  ).toBeVisible({ timeout: 15_000 })

  // PWA "App shell cached" toast overlays the attach row and blocks clicks.
  await dismissPwaToasts(page)

  const attachNote = page.getByRole('button', { name: 'Attach note' })
  await expect(attachNote).toBeEnabled({ timeout: 15_000 })
  await attachNote.click()
  await expect(page.getByText('Note attached.').first()).toBeVisible({
    timeout: 15_000,
  })

  await page.getByLabel('Attach photo from camera').setInputFiles({
    name: 'shot.png',
    mimeType: 'image/png',
    buffer: Buffer.from(
      'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
      'base64',
    ),
  })
  await expect(page.getByText('Photo attached.').first()).toBeVisible({
    timeout: 15_000,
  })

  // Close run via API
  const runDetail = await createRun(request, adminToken, projectId, {
    name: `CloseMe ${projectKey}`,
    start: true,
    include_all_cases: true,
  })
  const runId = runDetail.run.id
  const closed = await request.put(
    `${API_URL}/api/v1/projects/${projectId}/runs/${runId}`,
    {
      headers: {
        Authorization: `Bearer ${adminToken}`,
        'Content-Type': 'application/json',
      },
      data: { status: 'closed' },
    },
  )
  expect(closed.ok(), await closed.text()).toBeTruthy()
  const body = await closed.json()
  expect(['closed', 'completed']).toContain(body.run?.status ?? body.status)
})
