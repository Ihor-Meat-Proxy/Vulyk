import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import { loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { AdminPage } from '../pages/AdminPage'

test.describe('Admin content backups', () => {
  test('create local destination, back up now, see restore preview', async ({
    page,
  }) => {
    await loginAsAdmin(page)
    const admin = new AdminPage(page)
    await admin.open()
    await page.getByTestId(TestIds.adminTabBackups).click()
    await expect(page.getByTestId(TestIds.adminBackupsPanel)).toBeVisible()

    const destName = `Local e2e ${uniqueKey('bk')}`
    const before = await page.getByTestId(TestIds.adminBackupsDestRow).count()
    await page.getByTestId(TestIds.adminBackupsDestName).fill(destName)
    await page.getByTestId(TestIds.adminBackupsDestKind).selectOption('local')
    await page
      .getByTestId(TestIds.adminBackupsDestPath)
      .fill('./data/content-backups-e2e')
    await page.getByTestId(TestIds.adminBackupsDestSave).click()

    await expect(page.getByTestId(TestIds.adminBackupsDestRow)).toHaveCount(
      before + 1,
      { timeout: 15_000 },
    )

    const runDest = page.getByTestId(TestIds.adminBackupsRunDest)
    await expect
      .poll(async () => runDest.locator('option').count())
      .toBeGreaterThan(1)
    const optionCount = await runDest.locator('option').count()
    await runDest.selectOption({ index: optionCount - 1 })
    await page.getByTestId(TestIds.adminBackupsRunNow).click()

    await expect
      .poll(
        async () => {
          const statuses = await page
            .getByTestId(TestIds.adminBackupsRunStatus)
            .allTextContents()
          return statuses.some((s) => s.trim() === 'success')
        },
        { timeout: 60_000 },
      )
      .toBeTruthy()

    await page.getByTestId(TestIds.adminBackupsRestore).first().click()
    await expect(page.getByTestId(TestIds.adminBackupsRestorePreview)).toBeVisible({
      timeout: 30_000,
    })
    await expect(page.getByTestId(TestIds.adminBackupsRestoreCounts)).toBeVisible()
  })
})
