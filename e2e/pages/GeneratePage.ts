import { expect, type Page } from '@playwright/test'

export class GeneratePage {
  constructor(readonly page: Page) {}

  async open() {
    await this.page
      .getByRole('navigation', { name: 'Primary' })
      .getByRole('link', { name: /^Generate|Генерація$/i })
      .click()
    await expect(
      this.page.getByRole('heading', { name: /^Generate$|^Генерація$/i }),
    ).toBeVisible()
  }

  modelSelect(which: 'cases' | 'automation' = 'cases') {
    const label =
      which === 'automation'
        ? /Model|Модель|Automation model|Модель для автоматизації/i
        : /^Model$|^Модель$/i
    return this.page.locator('form.stack-form').getByLabel(label).first()
  }

  async generateDrafts(requirements: string) {
    await this.page
      .getByPlaceholder(/Describe the behaviour|Опишіть поведінку/i)
      .fill(requirements)
    await this.page
      .getByRole('button', {
        name: /Generate & save drafts|Згенерувати і зберегти чернетки/i,
      })
      .click()
  }

  async pickFirstCase() {
    const checkbox = this.page
      .locator('.case-picker-list input[type="checkbox"]')
      .first()
    await expect(checkbox).toBeVisible({ timeout: 20_000 })
    await checkbox.check()
  }

  async generateCode() {
    await this.page
      .getByRole('button', { name: /Generate code|Згенерувати код/i })
      .click()
  }
}
