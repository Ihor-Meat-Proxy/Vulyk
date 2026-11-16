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

test('Updates tab shows version and check control', {
  tag: ['@VQA-227', '@VUE-5'], annotation: [{ type: 'vulyk', description: 'VQA-227' }, { type: 'vulyk', description: 'VUE-5' }],
}, async ({ page }) => {
  await openUpdates(page)
  const current = page.getByTestId(TestIds.adminUpdatesCurrentVersion)
  await expect(current).toBeVisible()
  await expect(current).not.toHaveText('')
  await expect(page.getByTestId(TestIds.adminUpdatesLatestVersion)).toBeVisible()
  await expect(page.getByTestId(TestIds.adminUpdatesCheckButton)).toBeEnabled()
})

test('manual install and rollback commands without sidecar', {
  tag: ['@VQA-228', '@VUE-3'], annotation: [{ type: 'vulyk', description: 'VQA-228' }, { type: 'vulyk', description: 'VUE-3' }],
}, async ({
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

test('save update check interval preference', {
  tag: ['@VQA-229', '@VUE-4'], annotation: [{ type: 'vulyk', description: 'VQA-229' }, { type: 'vulyk', description: 'VUE-4' }],
}, async ({ page, request }) => {
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

test('version backups list and job history regions render', {
  tag: ['@VQA-230', '@VUE-6'], annotation: [{ type: 'vulyk', description: 'VQA-230' }, { type: 'vulyk', description: 'VUE-6' }],
}, async ({
  page,
}) => {
  await openUpdates(page)
  await expect(page.getByTestId(TestIds.adminUpdatesBackupList)).toBeVisible()
  await expect(page.getByTestId(TestIds.adminUpdatesJobList)).toBeVisible()
})

test('failed update check surfaces error instead of latest', {
  tag: ['@VQA-339', '@VUE-113'],
  annotation: [
    { type: 'vulyk', description: 'VQA-339' },
    { type: 'vulyk', description: 'VUE-113' },
  ],
}, async ({ page }) => {
  await openUpdates(page)

  // Prefer durable status fields (200 + last_check_error_*) over thrown API errors.
  // Match ?force=true (Check/Retry bypasses backoff via query string).
  await page.route('**/api/v1/admin/updates/check*', async (route) => {
    if (route.request().method() !== 'POST') {
      await route.continue()
      return
    }
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        current_version: '0.0.0-test',
        latest_version: '0.1.0',
        latest_notes: 'cached',
        latest_url: 'https://example.com/release',
        update_available: true,
        last_checked_at: '2024-01-01T00:00:00Z',
        last_check_error_code: 'git_host_rate_limit',
        last_check_error_at: '2024-06-01T12:00:00Z',
        last_check_error_reset_at: '2024-06-01T13:00:00Z',
        check_interval_hours: 24,
        capability: 'manual_only',
        manual_command: './scripts/install.sh upgrade',
        rollback_command: './scripts/install.sh rollback',
        github_repo: 'Ihor-Meat-Proxy/Vulyk',
        active_job: null,
        rollback: null,
        version_backups: [],
        retention: { rollback_days: 7, version_count: 3 },
      }),
    })
  })

  await page.getByTestId(TestIds.adminUpdatesCheckButton).click()

  const checkError = page.getByTestId(TestIds.adminUpdatesCheckError)
  await expect(checkError).toBeVisible({ timeout: 15_000 })
  await expect(checkError).toContainText(
    /rate limit|ліміт|limite|límite|Ratenlimit/i,
  )

  const status = page.getByTestId(TestIds.adminUpdatesStatus)
  await expect(status).toBeVisible()
  await expect(status).toContainText(
    /failed|не вдалася|Échec|Falló|fehlgeschlagen/i,
  )
  await expect(status).not.toContainText(
    /latest checked release|остання перевірена|dernière version vérifiée|última versión comprobada|zuletzt geprüfte/i,
  )
  // Cached last-good latest version remains visible.
  await expect(page.getByTestId(TestIds.adminUpdatesLatestVersion)).toContainText(
    '0.1.0',
  )
  // Check failures stay in the Updates panel alert only (no duplicate Admin banner).
  await expect(page.getByTestId(TestIds.adminError)).toHaveCount(0)
  // Retry reuses the check control.
  await expect(page.getByTestId(TestIds.adminUpdatesRetryButton)).toBeEnabled()
})
