import { expect, type Page } from '@playwright/test'

export class AdminPage {
  constructor(readonly page: Page) {}

  async open() {
    await this.page.getByRole('navigation', { name: 'Primary' }).getByRole('link', { name: /Admin|Адмін/i }).click()
    await expect(this.page.getByRole('heading', { name: /^Admin$|Адмін/i })).toBeVisible()
  }

  async openTab(name: string | RegExp) {
    await this.page.getByRole('tab', { name }).click()
  }

  async createUser(email: string, displayName: string, password: string) {
    await this.openTab(/Users|Користувачі/i)
    await this.page.locator('form.stack-form').getByLabel(/^Email$/i).fill(email)
    await this.page.getByLabel(/Display name/i).fill(displayName)
    await this.page.locator('form.stack-form').getByLabel(/^Password$/i).fill(password)
    await this.page.getByRole('button', { name: 'Create user' }).click()
    await expect(this.page.getByText(email)).toBeVisible({ timeout: 20_000 })
  }

  async assignRole(userEmail: string, roleLabel: string) {
    await this.openTab(/Roles|Ролі/i)
    await this.page.locator('form.stack-form select').first().selectOption({ label: userEmail })
    await this.page.locator('form.stack-form').getByLabel(/^Role$/i).selectOption({ label: roleLabel })
    await this.page.getByRole('button', { name: 'Save membership' }).click()
    await expect(this.page.getByText(userEmail)).toBeVisible()
  }

  async createProject(name: string, key?: string) {
    await this.openTab(/Projects|Проєкти/i)
    await this.page.locator('form.stack-form').getByLabel(/^Name$/i).fill(name)
    if (key) {
      await this.page
        .locator('form.stack-form')
        .getByLabel(/^Key \(optional\)$|^Key$/i)
        .fill(key)
    }
    await this.page.getByRole('button', { name: 'Create project' }).click()
    await expect(
      this.page.locator('.admin').getByText(name, { exact: true }).first(),
    ).toBeVisible({ timeout: 20_000 })
  }

  async openLlmTab() {
    await this.openTab(/LLM/i)
    await expect(this.page.getByRole('heading', { name: /LLM providers/i })).toBeVisible()
  }

  async openIntegrationsTab() {
    await this.openTab(/Integrations|Інтеграції/i)
  }

  async openSettingsTab() {
    await this.openTab(/Settings|Налаштування/i)
  }

  async openSsoTab() {
    await this.openTab(/^SSO$/i)
  }

  async openTokensTab() {
    await this.openTab(/API tokens|Tokens|Токени/i)
  }
}
