import { expect, type Page } from '@playwright/test'

export class SetupPage {
  constructor(readonly page: Page) {}

  async expectWizard() {
    // Clear any prior session so setup_required is not masked by auth.
    await this.page.goto('/')
    await this.page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })
    await this.page.goto('/')
    // Match the token field (title alone also appears on the success panel).
    await expect(
      this.page.getByLabel(/Setup token|Setup-токен/i),
    ).toBeVisible({ timeout: 60_000 })
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
    await this.page
      .getByLabel(/Display name|Ім'я/i)
      .fill(opts.displayName ?? 'Setup Admin')
    await this.page
      .getByLabel(/Admin email|Email адміна/i)
      .fill(opts.email)
    await this.page
      .getByLabel(/Admin password|Пароль адміна/i)
      .fill(opts.password)
    if (opts.locale) {
      await this.page
        .getByLabel(/Default locale|Мова за замовчуванням/i)
        .selectOption(opts.locale)
    }
    if (opts.timezone) {
      await this.page
        .getByLabel(/Default timezone|Часовий пояс/i)
        .fill(opts.timezone)
    }
    if (opts.registrationMode) {
      await this.page
        .getByLabel(/Registration mode|Режим реєстрації/i)
        .selectOption(opts.registrationMode)
    }
  }

  async submit() {
    await this.page
      .getByRole('button', { name: /Complete setup|Завершити налаштування/i })
      .click()
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
