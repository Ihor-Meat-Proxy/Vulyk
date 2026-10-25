import { expect, type Locator, type Page } from '@playwright/test'

export class LoginPage {
  readonly page: Page
  readonly panel: Locator
  readonly email: Locator
  readonly password: Locator
  readonly submit: Locator
  readonly error: Locator

  constructor(page: Page) {
    this.page = page
    this.panel = page.locator('.login-panel')
    this.email = page.getByLabel(/^Email$/i)
    this.password = page.getByLabel(/Password|Пароль/i)
    this.submit = page.getByRole('button', {
      name: /Sign in|Увійти|Create admin|Register/i,
    })
    this.error = page.locator('.login-panel .error')
  }

  async goto() {
    await this.page.goto('/')
    await expect(this.panel.or(this.page.locator('.app-shell')).first()).toBeVisible({
      timeout: 60_000,
    })
  }

  async login(email: string, password: string) {
    await this.goto()
    if (await this.page.locator('.app-shell').isVisible()) return
    await this.email.fill(email)
    await this.password.fill(password)
    await this.submit.click()
    await expect(this.page.locator('.app-shell')).toBeVisible({ timeout: 60_000 })
  }

  async expectLoginVisible() {
    await expect(this.panel).toBeVisible()
  }

  async switchToRegister() {
    await this.page.getByRole('button', { name: /Create an account/i }).click()
  }

  async register(email: string, password: string, displayName: string) {
    await this.switchToRegister()
    await this.page.getByLabel(/Display name/i).fill(displayName)
    await this.email.fill(email)
    await this.password.fill(password)
    await this.page.getByRole('button', { name: /^Register$/i }).click()
  }
}
