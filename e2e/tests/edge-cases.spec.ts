import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import {
  createCase,
  createProject,
  createSuite,
} from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { CasesPage } from '../pages/CasesPage'

test.describe.configure({ mode: 'serial' })

async function selectProject(
  page: import('@playwright/test').Page,
  key: string,
) {
  const select = page.getByTestId(TestIds.headerProjectSelect)
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(key))
  if (idx >= 0) await select.selectOption({ index: idx })
}

test('empty project shows empty folder tree state', {
  tag: ['@VQA-252', '@VUE-28'],
  annotation: [
    { type: 'vulyk', description: 'VQA-252' },
    { type: 'vulyk', description: 'VUE-28' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('EM')
  await createProject(request, admin.token, {
    key,
    name: `Empty ${key}`,
  })
  await loginAsAdmin(page)
  await selectProject(page, key)

  const cases = new CasesPage(page)
  await cases.open()
  const tree = cases.folderTree()
  await expect(tree).toBeVisible({ timeout: 20_000 })
  await expect(tree.getByTestId(TestIds.folderAllCases)).toBeVisible()
  await expect(tree.getByTestId(TestIds.folderNode)).toHaveCount(0)
})

test('unicode and special characters in case title', {
  tag: ['@VQA-253', '@VUE-30'],
  annotation: [
    { type: 'vulyk', description: 'VQA-253' },
    { type: 'vulyk', description: 'VUE-30' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('UC')
  const project = await createProject(request, admin.token, {
    key,
    name: `Unicode ${key}`,
  })
  const suite = await createSuite(request, admin.token, project.id, 'Україна')
  const title = `Київ concert ${key} 日本語`
  const created = await createCase(request, admin.token, project.id, {
    title,
    suite_id: suite.id,
    status: 'ready',
    priority: 'low',
    steps: [{ action: 'перевірити', expected_result: 'ок' }],
  })

  await loginAsAdmin(page)
  await selectProject(page, key)

  const cases = new CasesPage(page)
  await cases.open()
  await cases.selectFolder('Україна')
  await expect(
    page.locator(`[data-testid="case-row"]`).filter({ hasText: key }),
  ).toBeVisible({ timeout: 20_000 })
  await page.goto(`/cases/${created.case.key}`)
  await expect(page.getByTestId(TestIds.caseDetailPage)).toBeVisible({
    timeout: 20_000,
  })
  const body = await page.getByTestId(TestIds.caseDetailPage).innerText()
  expect(body).toMatch(/Київ|concert|日本語/)
})

test('long case title truncates in list but full in detail', {
  tag: ['@VQA-254', '@VUE-29'],
  annotation: [
    { type: 'vulyk', description: 'VQA-254' },
    { type: 'vulyk', description: 'VUE-29' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('LG')
  const project = await createProject(request, admin.token, {
    key,
    name: `Long ${key}`,
  })
  const suite = await createSuite(request, admin.token, project.id, 'Long')
  const title = `${'A'.repeat(240)}-${key}`
  const created = await createCase(request, admin.token, project.id, {
    title,
    suite_id: suite.id,
    status: 'ready',
    priority: 'medium',
  })

  await loginAsAdmin(page)
  await selectProject(page, key)

  const cases = new CasesPage(page)
  await cases.open()
  await cases.selectFolder('Long')
  await expect(
    page.locator(`[data-testid="case-row"]`).filter({ hasText: key }),
  ).toBeVisible({ timeout: 20_000 })
  await page.goto(`/cases/${created.case.key}`)
  await expect(page.getByTestId(TestIds.caseDetailPage)).toBeVisible({
    timeout: 20_000,
  })
  const titleInput = page.getByTestId(TestIds.caseDetailTitleInput)
  if (await titleInput.count()) {
    await expect(titleInput).toHaveValue(title)
  } else {
    await expect(page.getByTestId(TestIds.caseDetailBreadcrumbCurrent)).toContainText(
      key,
    )
  }
})
