import { expect, type Page } from '@playwright/test'

export class ExecutionsPage {
  constructor(readonly page: Page) {}

  async open() {
    await this.page
      .getByRole('navigation', { name: 'Primary' })
      .getByRole('link', { name: /Executions|Виконання/i })
      .click()
    await expect(this.page.locator('.executions-layout')).toBeVisible({
      timeout: 30_000,
    })
  }

  async selectIteration(name: string | RegExp) {
    const tree = this.page.locator('.exec-tree')
    await tree.getByRole('button', { name }).first().click()
  }

  async expectDashboard() {
    await expect(this.page.locator('.exec-donut')).toBeVisible({
      timeout: 20_000,
    })
  }

  async expectSuites() {
    await expect(
      this.page.getByRole('heading', {
        name: /Automated suites|Автоматичні набори/i,
      }),
    ).toBeVisible({ timeout: 15_000 })
  }

  async openFirstSuiteDetails() {
    await this.page
      .getByRole('button', { name: /Details|Деталі/i })
      .first()
      .click()
    await expect(
      this.page.getByRole('dialog').or(this.page.locator('.run-details-drawer')),
    ).toBeVisible({ timeout: 15_000 })
  }
}
