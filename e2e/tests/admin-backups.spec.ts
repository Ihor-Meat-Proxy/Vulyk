import { expect, test } from '@playwright/test'
import { loginAsAdmin } from '../fixtures/auth'
import { AdminPage } from '../pages/AdminPage'

test.describe('Admin content backups', () => {
  test('create local destination, back up now, see history', async ({
    page,
  }) => {
    await loginAsAdmin(page)
    const admin = new AdminPage(page)
    await admin.open()
    await page.getByTestId('admin-tab-backups').click()
    await expect(page.getByTestId('admin-backups-panel')).toBeVisible()

    const destName = `Local e2e ${Date.now()}`
    await page.getByTestId('admin-backups-dest-name').fill(destName)
    await page.getByTestId('admin-backups-dest-kind').selectOption('local')
    await page
      .getByTestId('admin-backups-dest-path')
      .fill('./data/content-backups-e2e')
    await page.getByTestId('admin-backups-dest-save').click()

    const destRow = page
      .getByTestId('admin-backups-destinations')
      .getByTestId('admin-backups-dest-local')
      .filter({ hasText: destName })
    await expect(destRow).toBeVisible({ timeout: 15_000 })

    await page
      .getByTestId('admin-backups-run-dest')
      .selectOption({ label: destName })
    await page.getByTestId('admin-backups-run-now').click()

    await expect
      .poll(
        async () => {
          const statuses = await page
            .getByTestId('admin-backups-run-status')
            .allTextContents()
          return statuses.some((s) => s.trim() === 'success')
        },
        { timeout: 60_000 },
      )
      .toBeTruthy()

    await page.getByTestId('admin-backups-restore').first().click()
    await expect(page.getByTestId('admin-backups-restore-preview')).toBeVisible({
      timeout: 30_000,
    })
  })
})
