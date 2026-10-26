import { expect, test } from '@playwright/test'
import {
  dismissPwaToasts,
  ensureAdminSession,
  loginAsAdmin,
} from '../fixtures/auth'
import { API_URL, uniqueKey } from '../fixtures/env'
import { ExecutionsPage } from '../pages/ExecutionsPage'

test.describe.configure({ mode: 'serial' })

let projectId = ''
let projectKey = ''
let adminToken = ''
test.beforeAll(async ({ request }) => {
  const admin = await ensureAdminSession(request)
  adminToken = admin.token
  projectKey = uniqueKey('EX')

  const project = await request.post(`${API_URL}/api/v1/projects`, {
    headers: { Authorization: `Bearer ${adminToken}` },
    data: { key: projectKey, name: `Executions ${projectKey}` },
  })
  expect(project.ok()).toBeTruthy()
  const projectBody = await project.json()
  projectId = projectBody.id

  const caseRes = await request.post(
    `${API_URL}/api/v1/projects/${projectId}/cases`,
    {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: {
        title: `${projectKey}-1 landing loads`,
        status: 'ready',
      },
    },
  )
  expect(caseRes.ok()).toBeTruthy()
  const caseDetail = await caseRes.json()
  const caseBody = caseDetail.case ?? caseDetail

  const campaign = await request.post(
    `${API_URL}/api/v1/projects/${projectId}/campaigns`,
    {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: { name: 'Integration_Automation' },
    },
  )
  expect(campaign.ok()).toBeTruthy()
  const campaignBody = await campaign.json()

  const iteration = await request.post(
    `${API_URL}/api/v1/projects/${projectId}/campaigns/${campaignBody.id}/iterations`,
    {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: {
        name: 'Daily - e2e',
        reference: 'Daily',
        mode: 'automated',
        cadence: 'daily',
        case_ids: [caseBody.id],
      },
    },
  )
  expect(iteration.ok()).toBeTruthy()
  await iteration.json()

  const caseKey = caseBody.key as string
  const junit = `<?xml version="1.0"?>
<testsuite name="pw" tests="2" failures="1">
  <testcase classname="a.spec.ts" name="${caseKey} landing loads" time="1.2"/>
  <testcase classname="a.spec.ts" name="orphan fail" time="0.3">
    <failure message="boom">stack</failure>
  </testcase>
</testsuite>`

  const ingest = await request.post(
    `${API_URL}/api/v1/projects/${projectId}/ingest`,
    {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: {
        format: 'junit',
        content: junit,
        campaign: 'Integration_Automation',
        iteration: 'Daily - e2e',
        created_by_name: 'WALL-E-rror',
        environment: { runner: 'Playwright' },
        artifacts: [
          {
            kind: 'report',
            label: 'HTML',
            url: 'https://example.invalid/report',
          },
        ],
      },
    },
  )
  expect(ingest.ok()).toBeTruthy()
})

test('executions tree, dashboard, suites drawer', async ({ page }) => {
  await loginAsAdmin(page)
  await dismissPwaToasts(page)

  const select = page.locator('.project-select')
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })

  const executions = new ExecutionsPage(page)
  await executions.open()
  await executions.selectIteration(/Daily - e2e/)
  await executions.expectDashboard()

  // Status tokens with labels
  await expect(page.locator('.status-pill').first()).toBeVisible()

  await executions.expectSuites()
  await expect(page.getByText('WALL-E-rror')).toBeVisible()
  await executions.openFirstSuiteDetails()
})

test('case executions tab shows history sparkline', async ({ page, request }) => {
  await loginAsAdmin(page)
  await dismissPwaToasts(page)

  const cases = await request.get(
    `${API_URL}/api/v1/projects/${projectId}/cases?limit=5`,
    { headers: { Authorization: `Bearer ${adminToken}` } },
  )
  const body = await cases.json()
  const caseId = body.items?.[0]?.id ?? body[0]?.id
  expect(caseId).toBeTruthy()

  const select = page.locator('.project-select')
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })

  await page.goto(`/cases/${caseId}`)
  await page.getByRole('tab', { name: /Executions|Виконання/i }).click()
  await expect(page.locator('.exec-sparkline')).toBeVisible({
    timeout: 20_000,
  })
  await expect(page.getByText(/Pass rate/i)).toBeVisible()
})
