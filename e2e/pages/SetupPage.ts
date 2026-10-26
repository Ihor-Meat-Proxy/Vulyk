import { expect, type Page } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'

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
    await expect(this.page.getByTestId(TestIds.setupTokenInput)).toBeVisible({
      timeout: 60_000,
    })
  }

  async fill(opts: {
    token: string
    email: string
    password: string
    displayName?: string
    locale?: 'en' | 'uk' | 'fr' | 'es' | 'de'
    timezone?: string
    registrationMode?: 'open' | 'invite' | 'disabled'
  }) {
    await this.page.getByTestId(TestIds.setupTokenInput).fill(opts.token)
    await this.page
      .getByTestId(TestIds.setupDisplayNameInput)
      .fill(opts.displayName ?? 'Setup Admin')
    await this.page.getByTestId(TestIds.setupEmailInput).fill(opts.email)
    await this.page.getByTestId(TestIds.setupPasswordInput).fill(opts.password)
    if (opts.locale) {
      await this.page
        .getByTestId(TestIds.setupLocaleSelect)
        .selectOption(opts.locale)
    }
    if (opts.timezone) {
      await this.page
        .getByTestId(TestIds.setupTimezoneInput)
        .fill(opts.timezone)
    }
    if (opts.registrationMode) {
      await this.page
        .getByTestId(TestIds.setupRegistrationSelect)
        .selectOption(opts.registrationMode)
    }
  }

  async submit() {
    await this.page.getByTestId(TestIds.setupSubmitButton).click()
  }

  async expectSuccess() {
    await expect(this.page.getByTestId(TestIds.setupSuccess)).toBeVisible({
      timeout: 30_000,
    })
  }

  async expectError() {
    await expect(this.page.getByTestId(TestIds.setupError)).toBeVisible({
      timeout: 15_000,
    })
  }
}
