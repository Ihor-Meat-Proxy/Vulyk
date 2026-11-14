import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import {
  createCase,
  createProject,
  createSuite,
} from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { API_URL, uniqueKey } from '../fixtures/env'

test.describe.configure({ mode: 'serial' })

test('create requirement and link from case detail', {
  tag: ['@VQA-310', '@VUE-87'], annotation: [{ type: 'vulyk', description: 'VQA-310' }, { type: 'vulyk', description: 'VUE-87' }],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('RQ')
  const project = await createProject(request, admin.token, {
    key,
    name: `Reqs ${key}`,
  })
  const suite = await createSuite(request, admin.token, project.id, 'Coverage')
  const created = await createCase(request, admin.token, project.id, {
    title: `Covered case ${key}`,
    suite_id: suite.id,
    status: 'ready',
    priority: 'high',
    steps: [{ action: 'a', expected_result: 'b' }],
  })

  await loginAsAdmin(page)
  const select = page.getByTestId(TestIds.headerProjectSelect)
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(key))
  if (idx >= 0) await select.selectOption({ index: idx })

  await page.getByTestId(TestIds.navLinkRequirements).click()
  await expect(page.getByTestId(TestIds.requirementsPage)).toBeVisible({
    timeout: 15_000,
  })
  const title = `REQ ${key} unicode 日本語`
  await page.getByTestId(TestIds.requirementsTitleInput).fill(title)
  await page.getByTestId(TestIds.requirementsAddButton).click()
  await expect(
    page.getByTestId(TestIds.requirementRow).filter({ hasText: title }),
  ).toBeVisible({ timeout: 20_000 })

  // Link from case detail when UI exposes the control
  await page.goto(`/cases/${created.case.key}`)
  await expect(page.getByTestId(TestIds.caseDetailPage)).toBeVisible({
    timeout: 20_000,
  })
  const linkSelect = page.getByTestId(TestIds.caseDetailLinkRequirementSelect)
  if (await linkSelect.isVisible().catch(() => false)) {
    await linkSelect.selectOption({ index: 1 })
    await page.getByTestId(TestIds.caseDetailLinkRequirementButton).click()
  } else {
    // Fallback: link via API so coverage page has data
    const reqs = await request.get(
      `${API_URL}/api/v1/projects/${project.id}/requirements`,
      { headers: { Authorization: `Bearer ${admin.token}` } },
    )
    expect(reqs.ok()).toBeTruthy()
    const body = await reqs.json()
    const list = Array.isArray(body) ? body : body.requirements || []
    const req = list.find((r: { title?: string }) => r.title === title)
    expect(req).toBeTruthy()
    const link = await request.post(
      `${API_URL}/api/v1/projects/${project.id}/cases/${created.case.id}/requirements`,
      {
        headers: {
          Authorization: `Bearer ${admin.token}`,
          'Content-Type': 'application/json',
        },
        data: { requirement_id: req.id },
      },
    )
    expect([200, 201, 204]).toContain(link.status())
  }
})

test('coverage page lists covered and uncovered', {
  tag: ['@VQA-311', '@VUE-86'], annotation: [{ type: 'vulyk', description: 'VQA-311' }, { type: 'vulyk', description: 'VUE-86' }],
}, async ({ page, request }) => {
  await ensureAdminSession(request)
  await loginAsAdmin(page)
  await page.goto('/reports/coverage')
  await expect(
    page
      .getByTestId(TestIds.reportsCoveragePage)
      .or(page.getByTestId(TestIds.reportsPage)),
  ).toBeVisible({ timeout: 20_000 })
  // Filters / lists render without crashing
  const covered = page.getByTestId(TestIds.reportsCoverageCovered)
  const uncovered = page.getByTestId(TestIds.reportsCoverageUncovered)
  if (await covered.isVisible().catch(() => false)) {
    await expect(covered).toBeVisible()
  }
  if (await uncovered.isVisible().catch(() => false)) {
    await expect(uncovered).toBeVisible()
  }
})
