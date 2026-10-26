import { expect, type Page } from '@playwright/test'

export class CasesPage {
  constructor(readonly page: Page) {}

  listPanel() {
    return this.page.locator('.case-list-panel')
  }

  async open() {
    await this.page.getByRole('navigation', { name: 'Primary' }).getByRole('link', { name: /Cases|Кейси/i }).click()
  }

  async createCase(title: string) {
    await this.page
      .getByLabel(
        /New case title|Назва нового кейсу|Titre du nouveau cas|Título del nuevo caso|Titel des neuen Falls/i,
      )
      .fill(title)
    await this.page.getByRole('button', { name: /Create case|Створити кейс/i }).click()
    await expect(this.listPanel().getByText(title)).toBeVisible({ timeout: 30_000 })
  }

  async search(query: string) {
    await this.page.getByLabel(/Search cases|Пошук кейсів/i).fill(query)
  }

  async filterStatus(status: string) {
    await this.page.getByLabel(/Status filter|Фільтр статусу/i).selectOption(status)
  }

  async selectCase(key: string) {
    await this.page.getByLabel(`Select ${key}`).check()
  }

  async bulkMarkReady() {
    await this.page.getByRole('button', { name: /Mark ready|Позначити готовим/i }).click()
  }

  async openCaseByTitle(title: string) {
    await this.listPanel().getByRole('link', { name: title }).click()
  }

  async backToCases() {
    await this.page.getByTestId('case-detail-back').click()
  }

  folderTree() {
    return this.page.locator('.folder-tree')
  }

  async selectFolder(name: string | RegExp) {
    await this.folderTree().getByRole('button', { name }).first().click()
  }

  async createFolder(name: string) {
    this.page.once('dialog', async (dialog) => {
      await dialog.accept(name)
    })
    await this.folderTree().getByRole('button', { name: /New folder|Нова тека/i }).click()
  }

  async editSteps(action: string, expected: string) {
    await this.page.getByRole('button', { name: 'Edit' }).click()
    const addStep = this.page.getByRole('button', { name: 'Add step' })
    if (await addStep.isVisible()) {
      await addStep.click()
    }
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
