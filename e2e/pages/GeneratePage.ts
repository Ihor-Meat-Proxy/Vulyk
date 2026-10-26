import { expect, type Page } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'

export class GeneratePage {
  constructor(readonly page: Page) {}

  async open() {
    await this.page.getByTestId(TestIds.navLinkGenerate).click()
    await expect(this.page.getByTestId(TestIds.generatePage)).toBeVisible()
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
    await this.page.getByTestId(TestIds.generateSubmitButton).click()
  }

  async pickFirstCase() {
    const checkbox = this.page
      .getByTestId(TestIds.casePicker)
      .getByTestId(TestIds.casePickerItem)
      .first()
      .locator('input[type="checkbox"]')
    await expect(checkbox).toBeVisible({ timeout: 20_000 })
    await checkbox.check()
  }

  async generateCode() {
    await this.page.getByTestId(TestIds.generateSubmitButton).click()
  }
}
