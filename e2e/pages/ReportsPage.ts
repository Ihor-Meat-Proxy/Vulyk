import { expect, type Page } from '@playwright/test'

export class ReportsPage {
  constructor(readonly page: Page) {}

  async open() {
    await this.page
      .getByRole('navigation', { name: 'Primary' })
      .getByRole('link', { name: /Reports|Звіти/i })
      .click()
    await expect(this.page.getByRole('heading', { name: /Reports/i })).toBeVisible()
  }

  async expectKpis() {
    await expect(this.page.getByText(/Recent run summaries/i)).toBeVisible()
    await expect(this.page.locator('.kpi-value').first()).toBeVisible()
  }

  async exportCsv() {
    const downloadPromise = this.page.waitForEvent('download')
    await this.page.getByRole('button', { name: 'Export CSV' }).click()
    const download = await downloadPromise
    expect(download.suggestedFilename()).toMatch(/\.csv$/i)
    return download
  }
}
