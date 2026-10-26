import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { API_URL } from '../fixtures/env'
import { AdminPage } from '../pages/AdminPage'

test.describe.configure({ mode: 'serial' })

test.beforeAll(async ({ request }) => {
  await ensureAdminSession(request)
})

async function openUpdates(page: import('@playwright/test').Page) {
  await loginAsAdmin(page)
  const admin = new AdminPage(page)
  await admin.open()
  await admin.openTab('updates')
  await expect(page.getByTestId(TestIds.adminUpdatesPanel)).toBeVisible({
    timeout: 20_000,
  })
}

test('Updates tab shows version and check control', async ({ page }) => {
  await openUpdates(page)
  const current = page.getByTestId(TestIds.adminUpdatesCurrentVersion)
  await expect(current).toBeVisible()
  await expect(current).not.toHaveText('')
  await expect(page.getByTestId(TestIds.adminUpdatesLatestVersion)).toBeVisible()
  await expect(page.getByTestId(TestIds.adminUpdatesCheckButton)).toBeEnabled()
})

test('manual install and rollback commands without sidecar', async ({
  page,
}) => {
  await openUpdates(page)
  const manual = page.getByTestId(TestIds.adminUpdatesManualCommand)
  const rollback = page.getByTestId(TestIds.adminUpdatesRollbackCommand)
  await expect(manual).toBeVisible()
  await expect(manual).toContainText(/install\.sh/i)
  await expect(rollback).toBeVisible()
  await expect(rollback).toContainText(/rollback/i)
  await expect(page.getByTestId(TestIds.adminUpdatesApplyButton)).toHaveCount(0)
  await expect(page.getByTestId(TestIds.adminUpdatesRollbackButton)).toHaveCount(
    0,
  )
})

test('save update check interval preference', async ({ page, request }) => {
  await openUpdates(page)
  const input = page.getByTestId(TestIds.adminUpdatesIntervalInput)
  await input.fill('12')
  await page.getByTestId(TestIds.adminUpdatesIntervalSaveButton).click()
  await expect(input).toHaveValue('12')

  const admin = await ensureAdminSession(request)
  await expect
    .poll(async () => {
      const res = await request.get(
        `${API_URL}/api/v1/admin/updates/preferences`,
        { headers: { Authorization: `Bearer ${admin.token}` } },
      )
      if (!res.ok()) return null
      const body = (await res.json()) as { check_interval_hours?: number }
      return body.check_interval_hours ?? null
    })
    .toBe(12)

  await page.getByTestId(TestIds.adminTabUpdates).click()
  await expect(page.getByTestId(TestIds.adminUpdatesIntervalInput)).toHaveValue(
    '12',
  )
})

test('version backups list and job history regions render', async ({
  page,
}) => {
  await openUpdates(page)
  await expect(page.getByTestId(TestIds.adminUpdatesBackupList)).toBeVisible()
  await expect(page.getByTestId(TestIds.adminUpdatesJobList)).toBeVisible()
})
