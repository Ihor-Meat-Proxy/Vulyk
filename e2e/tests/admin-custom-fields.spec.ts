import { expect, test } from '@playwright/test'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { AdminPage } from '../pages/AdminPage'
import { API_URL } from '../fixtures/env'

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

  // Seed via API (covered deeply by backend custom_fields_config test).
  const list = await apiJson(request, admin.token, 'POST', '/api/v1/admin/lists', {
    key: `browsers_${key}`,
    name: 'Browsers',
    items: [
      { label: 'Chrome', key: 'chrome', sort_order: 0 },
      { label: 'Firefox', key: 'firefox', sort_order: 1 },
    ],
  })
  const listId = list.id as string

  await apiJson(request, admin.token, 'POST', '/api/v1/admin/custom-fields', {
    key: `automated_${key}`,
    name: 'Automated?',
    field_type: 'checkbox',
    entity: 'test_case',
    scope: 'all',
    default_value: false,
    show_in_list: true,
    show_in_filters: true,
  })

  await apiJson(request, admin.token, 'POST', '/api/v1/admin/custom-fields', {
    key: `browsers_field_${key}`,
    name: 'Browsers',
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
  await expect(page.locator('.admin .badge').filter({ hasText: `browsers_${key}` })).toBeVisible({
    timeout: 20_000,
  })
  await page
    .getByRole('button', { name: /Manage|Керувати|Gérer|Gestionar|Verwalten/i })
    .first()
    .click()
  await expect(page.getByText('Chrome', { exact: true }).first()).toBeVisible({
    timeout: 15_000,
  })

  // Add an item via the dedicated Add-item form in the detail pane.
  const addItemBtn =
    /Add item|Додати елемент|Ajouter un élément|Añadir elemento|Eintrag hinzufügen/i
  const addForm = page.locator('form').filter({ has: page.getByRole('button', { name: addItemBtn }) })
  await addForm.locator('input').fill('Safari')
  await addForm.getByRole('button', { name: addItemBtn }).click()
  await expect(page.getByText('Safari', { exact: true }).first()).toBeVisible({
    timeout: 15_000,
  })

  await adminPage.openCustomFieldsTab()
  await expect(page.locator('.admin').getByText('Automated?', { exact: true }).first()).toBeVisible({
    timeout: 20_000,
  })
  await expect(page.locator('.admin').getByText('Browsers', { exact: true }).first()).toBeVisible()

  await adminPage.openExportTab()
  await expect(
    page.getByText(
      /Export config|Експорт|Exporter la config|Exportar config|Konfiguration exportieren/i,
    ),
  ).toBeVisible()
  const downloadPromise = page.waitForEvent('download', { timeout: 25_000 })
  await page
    .getByRole('button', {
      name: /Download export|Завантажити|Télécharger|Descargar|Herunterladen/i,
    })
    .click()
  const download = await downloadPromise
  expect(download.suggestedFilename()).toMatch(/vulyk-config\.(json|yaml)/)
})
