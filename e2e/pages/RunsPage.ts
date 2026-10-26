import { expect, type Page } from '@playwright/test'
import path from 'node:path'

export class RunsPage {
  constructor(readonly page: Page) {}

  async open() {
    await this.page
      .getByRole('navigation', { name: 'Primary' })
      .getByRole('link', {
        name: /Runs|Прогони|Exécutions de test|Ejecuciones de pruebas|Testläufe/i,
        exact: true,
      })
      .click()
    await expect(
      this.page.getByRole('heading', { name: 'Start a run', exact: true }),
    ).toBeVisible({ timeout: 30_000 })
  }

  async createPlan(name: string) {
    await this.page.getByPlaceholder('New plan name').fill(name)
    await this.page.getByRole('button', { name: 'Add plan' }).click()
    // Name appears in the plans list and as a <select> option (avoid getByText strict).
    await expect(
      this.page.locator('.history-list strong', { hasText: name }),
    ).toBeVisible({ timeout: 20_000 })
  }

  async createRun(name: string) {
    await this.page.getByPlaceholder('Run name').fill(name)
    await this.page.getByRole('button', { name: 'Create run' }).click()
    await expect(this.page.getByRole('heading', { name: name })).toBeVisible({
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
    await this.page.getByRole('link', { name }).click()
    await expect(this.page.getByRole('heading', { name })).toBeVisible()
  }

  async executeStatus(status: 'Passed' | 'Failed' | 'Blocked' | 'Skipped') {
    const list = this.page.getByRole('list', { name: 'Run cases' })
    await list.locator('button').first().click()
    await this.page
      .getByRole('group', { name: 'Result status' })
      .getByRole('button', { name: status, exact: true })
      .click()
    await expect(
      this.page.locator(`.status-pill.status-${status.toLowerCase()}`).first(),
    ).toBeVisible({ timeout: 20_000 })
  }
}
