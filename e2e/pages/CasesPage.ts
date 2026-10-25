import { expect, type Page } from '@playwright/test'

export class CasesPage {
  constructor(readonly page: Page) {}

  async open() {
    await this.page.getByRole('navigation', { name: 'Primary' }).getByRole('link', { name: /Cases|Кейси/i }).click()
  }

  async createCase(key: string, title: string) {
    await this.page.getByLabel('New case key').fill(key)
    await this.page.getByLabel('New case title').fill(title)
    await this.page.getByRole('button', { name: 'Create case' }).click()
    await expect(this.page.getByText(title)).toBeVisible({ timeout: 30_000 })
  }

  async search(query: string) {
    await this.page.getByLabel('Search cases').fill(query)
  }

  async filterStatus(status: string) {
    await this.page.locator('select.filter-input').selectOption(status)
  }

  async selectCase(key: string) {
    await this.page.getByLabel(`Select ${key}`).check()
  }

  async bulkMarkReady() {
    await this.page.getByRole('button', { name: 'Mark ready' }).click()
  }

  async openCaseByTitle(title: string) {
    await this.page.getByRole('link', { name: title }).click()
  }

  async editSteps(action: string, expected: string) {
    await this.page.getByRole('button', { name: 'Edit' }).click()
    const addStep = this.page.getByRole('button', { name: 'Add step' })
    if (await addStep.isVisible()) {
      await addStep.click()
    }
    const actionInputs = this.page.locator('.steps textarea, .steps input').filter({ hasText: '' })
    // Fill last step action / expected fields by placeholder or order
    const stepBlocks = this.page.locator('.step, .steps > div')
    const last = stepBlocks.last()
    const inputs = last.locator('textarea, input')
    const count = await inputs.count()
    if (count >= 2) {
      await inputs.nth(0).fill(action)
      await inputs.nth(1).fill(expected)
    } else {
      await this.page.getByPlaceholder(/action/i).last().fill(action).catch(async () => {
        await inputs.first().fill(action)
      })
    }
    await this.page.getByRole('button', { name: /Save/i }).click()
  }
}
