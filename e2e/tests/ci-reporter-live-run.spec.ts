import { expect, test } from '@playwright/test'
import {
  apiJson,
  createCase,
  createProject,
} from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'

test.describe.configure({ mode: 'serial' })

let projectKey = ''
let projectId = ''
let adminToken = ''
let caseKey = ''
let caseKeyB = ''

test.beforeAll(async ({ request }) => {
  const admin = await ensureAdminSession(request)
  adminToken = admin.token
  projectKey = uniqueKey('LR')
  const project = await createProject(request, admin.token, {
    key: projectKey,
    name: `LiveReporter ${projectKey}`,
  })
  projectId = project.id
  const a = await createCase(request, admin.token, projectId, {
    title: `Live A ${projectKey}`,
    status: 'ready',
  })
  caseKey = a.case.key
  const b = await createCase(request, admin.token, projectId, {
    title: `Live B ${projectKey}`,
    status: 'ready',
  })
  caseKeyB = b.case.key
})

test('open with start leaves run in_progress before results', {
  tag: ['@VQA-328', '@VUE-207'],
  annotation: [
    { type: 'vulyk', description: 'VQA-328' },
    { type: 'vulyk', description: 'VUE-207' },
  ],
}, async ({ request }) => {
  const opened = await apiJson<{
    run: { id: string; status: string }
  }>(request, 'POST', `/api/v1/projects/${projectId}/ci/runs`, {
    token: adminToken,
    data: {
      provider: 'other',
      suite_name: `Live begin ${projectKey}`,
      external_ref: `live-begin-${uniqueKey('')}`,
      start: true,
    },
  })
  expect(opened.run.status).toBe('in_progress')

  const got = await apiJson<{ id: string; status: string }>(
    request,
    'GET',
    `/api/v1/projects/${projectId}/ci/runs/${opened.run.id}`,
    { token: adminToken },
  )
  expect(got.status).toBe('in_progress')
})

test('streaming posts partial results while run stays in_progress', {
  tag: ['@VQA-329', '@VUE-208'],
  annotation: [
    { type: 'vulyk', description: 'VQA-329' },
    { type: 'vulyk', description: 'VUE-208' },
  ],
}, async ({ request }) => {
  const opened = await apiJson<{ run: { id: string; status: string } }>(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs`,
    {
      token: adminToken,
      data: {
        provider: 'other',
        suite_name: `Live stream ${projectKey}`,
        external_ref: `live-stream-${uniqueKey('')}`,
        start: true,
      },
    },
  )
  expect(opened.run.status).toBe('in_progress')

  const first = await apiJson<{ inserted: number }>(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs/${opened.run.id}/results`,
    {
      token: adminToken,
      data: {
        results: [
          {
            case_key: caseKey,
            status: 'passed',
            external_id: 'spec::a',
            duration_ms: 9,
          },
        ],
      },
    },
  )
  expect(first.inserted).toBe(1)

  const mid = await apiJson<{ status: string }>(
    request,
    'GET',
    `/api/v1/projects/${projectId}/ci/runs/${opened.run.id}`,
    { token: adminToken },
  )
  expect(mid.status).toBe('in_progress')

  const second = await apiJson<{ inserted: number }>(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs/${opened.run.id}/results`,
    {
      token: adminToken,
      data: {
        results: [
          {
            case_key: caseKeyB,
            status: 'failed',
            external_id: 'spec::b',
            duration_ms: 4,
          },
        ],
      },
    },
  )
  expect(second.inserted).toBe(1)

  await apiJson(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs/${opened.run.id}/close`,
    {
      token: adminToken,
      data: { message: 'stream done' },
    },
  )
})

test('close at end marks run closed', {
  tag: ['@VQA-330', '@VUE-209'],
  annotation: [
    { type: 'vulyk', description: 'VQA-330' },
    { type: 'vulyk', description: 'VUE-209' },
  ],
}, async ({ request }) => {
  const opened = await apiJson<{ run: { id: string } }>(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs`,
    {
      token: adminToken,
      data: {
        provider: 'other',
        suite_name: `Live close ${projectKey}`,
        external_ref: `live-close-${uniqueKey('')}`,
        start: true,
      },
    },
  )

  await apiJson(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs/${opened.run.id}/results`,
    {
      token: adminToken,
      data: {
        results: [
          {
            case_key: caseKey,
            status: 'passed',
            external_id: 'spec::close',
          },
        ],
      },
    },
  )

  const closed = await apiJson<{ status: string }>(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs/${opened.run.id}/close`,
    {
      token: adminToken,
      data: { message: 'suite finished' },
    },
  )
  expect(closed.status).toBe('closed')
})

test('cancelled close shows Cancelled status badge', {
  tag: ['@VQA-331', '@VUE-210'],
  annotation: [
    { type: 'vulyk', description: 'VQA-331' },
    { type: 'vulyk', description: 'VUE-210' },
  ],
}, async ({ page, request }) => {
  const suiteName = `Live cancel ${projectKey}`
  const opened = await apiJson<{ run: { id: string } }>(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs`,
    {
      token: adminToken,
      data: {
        provider: 'other',
        suite_name: suiteName,
        name: suiteName,
        external_ref: `live-cancel-${uniqueKey('')}`,
        start: true,
      },
    },
  )

  const cancelled = await apiJson<{
    status: string
    meta?: { cancelled?: boolean }
  }>(request, 'POST', `/api/v1/projects/${projectId}/ci/runs/${opened.run.id}/close`, {
    token: adminToken,
    data: { cancelled: true, message: 'playwright interrupted' },
  })
  expect(cancelled.status).toBe('cancelled')
  expect(cancelled.meta?.cancelled).toBe(true)

  await loginAsAdmin(page)
  const select = page.locator('.project-select')
  await expect
    .poll(async () => select.locator('option').count())
    .toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })

  await page.goto(`/runs/${opened.run.id}`)
  const badge = page.getByTestId('run-detail-status-badge')
  await expect(badge).toBeVisible({ timeout: 20_000 })
  await expect(badge).toHaveAttribute('data-status', 'cancelled')
  const label = (await badge.innerText()).trim()
  expect(label.toLowerCase()).toContain('cancel')
  expect(label).not.toMatch(/^(closed|passed)$/i)
})
