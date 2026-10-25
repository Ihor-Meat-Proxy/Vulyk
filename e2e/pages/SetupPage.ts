import { expect, type Page } from '@playwright/test'

export class SetupPage {
  constructor(readonly page: Page) {}

  async expectWizard() {
    await this.page.goto('/')
    await expect(this.page.getByText(/Instance setup|Налаштування інстансу/i)).toBeVisible({
      timeout: 60_000,
    })
  }

  async fill(opts: {
    token: string
    email: string
    password: string
    displayName?: string
    locale?: 'en' | 'uk'
    timezone?: string
    registrationMode?: 'open' | 'invite' | 'disabled'
  }) {
    await this.page.getByLabel(/Setup token|Setup-токен/i).fill(opts.token)
    await this.page.getByLabel(/Display name/i).fill(opts.displayName ?? 'Setup Admin')
    await this.page.getByLabel(/Admin email/i).fill(opts.email)
    await this.page.getByLabel(/Admin password/i).fill(opts.password)
    if (opts.locale) {
      await this.page.getByLabel(/Default locale/i).selectOption(opts.locale)
    }
    if (opts.timezone) {
      await this.page.getByLabel(/Default timezone/i).fill(opts.timezone)
    }
    if (opts.registrationMode) {
      await this.page.getByLabel(/Registration mode/i).selectOption(opts.registrationMode)
    }
  }

  async submit() {
    await this.page.getByRole('button', { name: /Complete setup/i }).click()
  }

  async expectSuccess() {
    await expect(
      this.page.getByText(/Setup complete|Налаштування завершено/i),
    ).toBeVisible({ timeout: 30_000 })
  }

  async expectError() {
    await expect(this.page.locator('.login-panel .error')).toBeVisible()
  }
}
