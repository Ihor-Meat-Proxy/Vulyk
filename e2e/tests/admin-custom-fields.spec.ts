import { expect, test } from '@playwright/test'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { AdminPage } from '../pages/AdminPage'
import { createProject } from '../fixtures/api'

test.describe.configure({ mode: 'serial' })

test('admin lists, custom fields, and config export', async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('cf').toLowerCase()
  await createProject(request, admin.token, {
    key: key.toUpperCase().replace(/[^a-z0-9]/gi, '').slice(0, 8) || 'CFTEST',
    name: `Custom fields ${key}`,
  })

  await loginAsAdmin(page)
  const adminPage = new AdminPage(page)
  await adminPage.open()

  // Lists
  await adminPage.openListsTab()
  const listKey = `browsers_${key}`.replace(/[^a-z0-9_]/g, '_').slice(0, 40)
  const createForm = page.locator('.stack-form form, form.stack-form').first()
  await createForm.locator('input').nth(0).fill(listKey)
  await createForm.locator('input').nth(1).fill('Browsers')
  await createForm
    .getByRole('button', {
      name: /Create list|Створити список|Créer|Crear|Liste erstellen/i,
    })
    .click()
  // Key appears in table badge and again in the detail header once selected.
  await expect(page.locator('.admin .badge').filter({ hasText: listKey }).first()).toBeVisible({
    timeout: 20_000,
  })

  await page
    .getByRole('button', { name: /Manage|Керувати|Gérer|Gestionar|Verwalten/i })
    .first()
    .click()
  const detailForm = page
    .locator('.stack-form')
    .filter({ hasText: /Add item|Додати|Ajouter|Añadir|Hinzufügen/i })
  await detailForm.locator('input').first().fill('Chrome')
  await detailForm
    .getByRole('button', {
      name: /Add item|Додати|Ajouter|Añadir|Hinzufügen/i,
    })
    .click()
  await expect(page.getByText('Chrome', { exact: true }).first()).toBeVisible({
    timeout: 15_000,
  })

  // Custom fields
  await adminPage.openCustomFieldsTab()
  const fieldKey = `automated_${key}`.replace(/[^a-z0-9_]/g, '_').slice(0, 40)
  const fieldForm = page
    .locator('form')
    .filter({
      hasText:
        /Create field|Створити поле|Créer un champ|Crear campo|Feld erstellen/i,
    })
    .first()
  await fieldForm.locator('input').first().fill(fieldKey)
  await fieldForm.locator('input').nth(1).fill('Automated?')
  const typeSelect = fieldForm.locator('select').first()
  if (await typeSelect.count()) {
    await typeSelect.selectOption('checkbox')
  }
  await fieldForm
    .getByRole('button', {
      name: /Create field|Створити поле|Créer|Crear|Feld erstellen/i,
    })
    .click()
  await expect(
    page.locator('.admin').getByText('Automated?', { exact: true }).first(),
  ).toBeVisible({ timeout: 20_000 })

  // Export config
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
