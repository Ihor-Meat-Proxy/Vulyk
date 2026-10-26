import { expect, type Page } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'

export class ExecutionsPage {
  constructor(readonly page: Page) {}

  async open() {
    await this.page.getByTestId(TestIds.navLinkExecutions).click()
    await expect(this.page.getByTestId(TestIds.executionsPage)).toBeVisible({
      timeout: 30_000,
    })
  }

  async selectIteration(name: string | RegExp) {
    await this.page
      .getByTestId(TestIds.executionsTree)
      .getByTestId(TestIds.executionNode)
      .filter({ hasText: name })
      .first()
      .click()
  }

  async expectDashboard() {
    await expect(
      this.page.getByTestId(TestIds.iterationDashboard),
    ).toBeVisible({
      timeout: 20_000,
    })
  }

  async expectSuites() {
    await expect(
      this.page.getByTestId(TestIds.automatedSuitesTable),
    ).toBeVisible({ timeout: 15_000 })
  }

  async openFirstSuiteDetails() {
    await this.page
      .getByTestId(TestIds.automatedSuiteRow)
      .first()
      .getByRole('button', { name: /Details|Деталі/i })
      .click()
    await expect(this.page.getByTestId(TestIds.runDetailsDrawer)).toBeVisible({
      timeout: 15_000,
    })
  }
}
