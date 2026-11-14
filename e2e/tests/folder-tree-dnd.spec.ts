import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import {
  apiJson,
  createCase,
  createProject,
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

test('folder tree create nested rename delete', {
  tag: ['@VQA-263', '@VUE-40'],
  annotation: [
    { type: 'vulyk', description: 'VQA-263' },
    { type: 'vulyk', description: 'VUE-40' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('FD')
  const project = await createProject(request, admin.token, {
    key,
    name: `Folders ${key}`,
  })
  const parent = await apiJson<{ id: string }>(
    request,
    'POST',
    `/api/v1/projects/${project.id}/suites`,
    { token: admin.token, data: { name: `Parent ${key}`, sort_order: 0 } },
  )
  const child = await apiJson<{ id: string }>(
    request,
    'POST',
    `/api/v1/projects/${project.id}/suites`,
    {
      token: admin.token,
      data: { name: `Child ${key}`, parent_id: parent.id, sort_order: 0 },
    },
  )

  await loginAsAdmin(page)
  await selectProject(page, key)

  const cases = new CasesPage(page)
  await cases.open()
  const tree = cases.folderTree()
  await expect(tree).toBeVisible({ timeout: 20_000 })
  await expect(tree.getByText(`Parent ${key}`)).toBeVisible({ timeout: 15_000 })

  const parentBtn = tree
    .locator(`[data-testid="${TestIds.folderNode}"]`)
    .filter({ hasText: `Parent ${key}` })
  await parentBtn.getByTestId(TestIds.folderNodeTwistie).click()
  await expect(tree.getByText(`Child ${key}`)).toBeVisible({ timeout: 15_000 })

  // Rename + delete via API; assert UI refreshes (prompt dialogs are flaky in Docker)
  await apiJson(
    request,
    'PUT',
    `/api/v1/projects/${project.id}/suites/${child.id}`,
    { token: admin.token, data: { name: `Renamed ${key}` } },
  )
  await page.reload()
  await selectProject(page, key)
  await cases.open()
  await expect(cases.folderTree().getByText(`Parent ${key}`)).toBeVisible({
    timeout: 15_000,
  })
  await cases
    .folderTree()
    .locator(`[data-testid="${TestIds.folderNode}"]`)
    .filter({ hasText: `Parent ${key}` })
    .getByTestId(TestIds.folderNodeTwistie)
    .click()
  await expect(cases.folderTree().getByText(`Renamed ${key}`)).toBeVisible({
    timeout: 15_000,
  })

  await apiJson(
    request,
    'DELETE',
    `/api/v1/projects/${project.id}/suites/${child.id}`,
    { token: admin.token, expectedStatus: 204 },
  )
  await page.reload()
  await selectProject(page, key)
  await cases.open()
  await expect(cases.folderTree().getByText(`Renamed ${key}`)).toHaveCount(0, {
    timeout: 15_000,
  })
})

test('drag case onto folder moves it', {
  tag: ['@VQA-264', '@VUE-39'],
  annotation: [
    { type: 'vulyk', description: 'VQA-264' },
    { type: 'vulyk', description: 'VUE-39' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('DN')
  const project = await createProject(request, admin.token, {
    key,
    name: `DnD ${key}`,
  })
  const suiteA = await apiJson<{ id: string; name: string }>(
    request,
    'POST',
    `/api/v1/projects/${project.id}/suites`,
    { token: admin.token, data: { name: `Alpha ${key}`, sort_order: 0 } },
  )
  const suiteB = await apiJson<{ id: string; name: string }>(
    request,
    'POST',
    `/api/v1/projects/${project.id}/suites`,
    { token: admin.token, data: { name: `Beta ${key}`, sort_order: 1 } },
  )
  const created = await createCase(request, admin.token, project.id, {
    title: `Drag me ${key}`,
    suite_id: suiteA.id,
    status: 'ready',
    priority: 'medium',
    steps: [{ action: 'x', expected_result: 'y' }],
  })

  await loginAsAdmin(page)
  await selectProject(page, key)

  const cases = new CasesPage(page)
  await cases.open()
  const tree = cases.folderTree()
  await expect(tree).toBeVisible({ timeout: 20_000 })

  const alpha = tree
    .locator(`[data-testid="${TestIds.folderNode}"]`)
    .filter({ hasText: `Alpha ${key}` })
  await alpha.getByTestId(TestIds.folderNodeTwistie).click()
  const caseItem = tree
    .getByTestId(TestIds.folderCaseItem)
    .filter({ hasText: `Drag me ${key}` })
  await expect(caseItem).toBeVisible({ timeout: 15_000 })

  const beta = tree
    .locator(`[data-testid="${TestIds.folderNode}"]`)
    .filter({ hasText: `Beta ${key}` })
  await caseItem.dragTo(beta.getByTestId(TestIds.folderNodeButton))

  await expect
    .poll(async () => {
      const detail = await apiJson<{ case: { suite_id: string | null } }>(
        request,
        'GET',
        `/api/v1/projects/${project.id}/cases/${created.case.id}`,
        { token: admin.token },
      )
      return detail.case.suite_id
    })
    .toBe(suiteB.id)
})
