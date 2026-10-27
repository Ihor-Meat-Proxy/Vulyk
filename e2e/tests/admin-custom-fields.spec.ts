import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey, API_URL } from '../fixtures/env'
import { AdminPage } from '../pages/AdminPage'

async function apiJson(
  request: import('@playwright/test').APIRequestContext,
  token: string,
  method: 'GET' | 'POST' | 'PUT' | 'DELETE',
  path: string,
  body?: unknown,
) {
  const res = await request.fetch(`${API_URL}${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${token}`,
      'Content-Type': 'application/json',
    },
    data: body,
  })
  expect(res.ok(), `${method} ${path} -> ${res.status()}`).toBeTruthy()
  if (res.status() === 204) return null
  return res.json()
}

test.describe.configure({ mode: 'serial' })

test('admin lists, custom fields, and config export', async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('cf').toLowerCase().replace(/[^a-z0-9_]/g, '_')
  const listKey = `browsers_${key}`
  const listName = `Browsers ${key}`
  const fieldName = `Automated ${key}?`

  const list = await apiJson(request, admin.token, 'POST', '/api/v1/admin/lists', {
    key: listKey,
    name: listName,
    items: [
      { label: 'Chrome', key: 'chrome', value: 'Chrome', sort_order: 0 },
      { label: 'Firefox', key: 'firefox', value: 'Firefox', sort_order: 1 },
    ],
  })
  const listId = list.id as string

  await apiJson(request, admin.token, 'POST', '/api/v1/admin/custom-fields', {
    key: `automated_${key}`,
    name: fieldName,
    field_type: 'checkbox',
    entity: 'test_case',
    scope: 'all',
    default_value: false,
    show_in_list: true,
    show_in_filters: true,
  })

  await apiJson(request, admin.token, 'POST', '/api/v1/admin/custom-fields', {
    key: `browsers_field_${key}`,
    name: `Browsers field ${key}`,
    field_type: 'multi_select',
    entity: 'test_case',
    option_list_id: listId,
    scope: 'all',
    default_value: [],
    show_in_list: true,
    show_in_filters: true,
  })

  await loginAsAdmin(page)
  const adminPage = new AdminPage(page)
  await adminPage.open()

  await adminPage.openListsTab()
  const listRow = page.locator('tr').filter({ hasText: listKey })
  await expect(listRow).toBeVisible({ timeout: 20_000 })
  await listRow.getByRole('button', { name: /Manage|Керувати|Gérer|Gestionar|Verwalten/i }).click()

  // Labels render as plain text; value equals label so no "(chrome)" suffix.
  await expect(page.getByRole('cell', { name: 'Chrome', exact: true })).toBeVisible({
    timeout: 15_000,
  })

  const addItemBtn =
    /Add item|Додати елемент|Ajouter un élément|Añadir elemento|Eintrag hinzufügen/i
  const addForm = page.locator('form').filter({ has: page.getByRole('button', { name: addItemBtn }) })
  await addForm.locator('input').fill('Safari')
  await addForm.getByRole('button', { name: addItemBtn }).click()
  await expect(page.getByRole('cell', { name: 'Safari', exact: true })).toBeVisible({
    timeout: 15_000,
  })

  await adminPage.openCustomFieldsTab()
  await expect(page.locator('.admin').getByText(fieldName, { exact: true })).toBeVisible({
    timeout: 20_000,
  })
  await expect(
    page.locator('.admin').getByText(`Browsers field ${key}`, { exact: true }),
  ).toBeVisible()

  await adminPage.openExportTab()
  await expect(page.getByTestId(TestIds.adminConfigExportButton)).toBeVisible()
  const downloadPromise = page.waitForEvent('download', { timeout: 25_000 })
  await page.getByTestId(TestIds.adminConfigExportButton).click()
  const download = await downloadPromise
  expect(download.suggestedFilename()).toMatch(/vulyk-config\.(json|yaml)/)
})
