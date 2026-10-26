import { expect, type Page } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'

export class ReportsPage {
  constructor(readonly page: Page) {}

  async open() {
    await this.page.getByTestId(TestIds.navLinkReports).click()
    await expect(this.page.getByTestId(TestIds.reportsPage)).toBeVisible()
  }

  async expectKpis() {
    await expect(this.page.getByText(/Recent run summaries/i)).toBeVisible()
    await expect(this.page.locator('.kpi-value').first()).toBeVisible()
  }

  async exportCsv() {
    const downloadPromise = this.page.waitForEvent('download')
    await this.page.getByTestId(TestIds.reportsExportButton).click()
    const download = await downloadPromise
    expect(download.suggestedFilename()).toMatch(/\.csv$/i)
    return download
  }
}
