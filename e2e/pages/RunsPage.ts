import { expect, type Page } from '@playwright/test'
import path from 'node:path'
import { TestIds } from '../../frontend/src/testing/testIds'

export class RunsPage {
  constructor(readonly page: Page) {}

  async open() {
    await this.page.getByTestId(TestIds.navLinkRuns).click()
    await expect(this.page.getByTestId(TestIds.runsPage)).toBeVisible({
      timeout: 30_000,
    })
  }

  async createPlan(name: string) {
    await this.page.getByPlaceholder('New plan name').fill(name)
    await this.page.getByRole('button', { name: 'Add plan' }).click()
    await expect(
      this.page.locator('.history-list strong', { hasText: name }),
    ).toBeVisible({ timeout: 20_000 })
  }

  async createRun(name: string) {
    await this.page.getByTestId(TestIds.runsCreateNameInput).fill(name)
    await this.page.getByTestId(TestIds.runsCreateButton).click()
    await expect(this.page.getByRole('heading', { name })).toBeVisible({
      timeout: 30_000,
    })
  }

  async uploadJunit(runName: string, filePath: string) {
    await this.open()
    await this.page.getByLabel('Upload target run').selectOption({ label: runName })
    await this.page.getByLabel('JUnit XML file').setInputFiles(path.resolve(filePath))
    await expect(this.page.getByText(/Ingested \d+ results/i)).toBeVisible({
      timeout: 30_000,
    })
  }

  async openRun(name: string) {
    await this.page
      .getByTestId(TestIds.runRow)
      .filter({ hasText: name })
      .getByTestId(TestIds.runRowName)
      .click()
    await expect(this.page.getByTestId(TestIds.runDetailPanel)).toBeVisible()
  }

  async executeStatus(status: 'Passed' | 'Failed' | 'Blocked' | 'Skipped') {
    const list = this.page.getByTestId(TestIds.runDetailCases)
    await list.getByTestId(TestIds.runDetailCaseRow).first().click()
    const map = {
      Passed: TestIds.executionPassButton,
      Failed: TestIds.executionFailButton,
      Blocked: TestIds.executionBlockButton,
      Skipped: TestIds.executionSkipButton,
    } as const
    await this.page.getByTestId(map[status]).click()
    await expect(
      this.page.locator(`.status-pill.status-${status.toLowerCase()}`).first(),
    ).toBeVisible({ timeout: 20_000 })
  }
}
