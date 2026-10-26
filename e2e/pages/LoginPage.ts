import { expect, type Locator, type Page } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'

export class LoginPage {
  readonly page: Page
  readonly panel: Locator
  readonly email: Locator
  readonly password: Locator
  readonly submit: Locator
  readonly error: Locator

  constructor(page: Page) {
    this.page = page
    this.panel = page.getByTestId(TestIds.loginPanel)
    this.email = page.getByTestId(TestIds.loginEmailInput)
    this.password = page.getByTestId(TestIds.loginPasswordInput)
    this.submit = page.getByTestId(TestIds.loginSubmitButton)
    this.error = page.getByTestId(TestIds.loginError)
  }

  async goto() {
    await this.page.goto('/')
    await expect(
      this.panel.or(this.page.getByTestId(TestIds.appShell)).first(),
    ).toBeVisible({
      timeout: 60_000,
    })
  }

  async login(email: string, password: string) {
    await this.goto()
    if (await this.page.getByTestId(TestIds.appShell).isVisible()) return
    await this.email.fill(email)
    await this.password.fill(password)
    await this.submit.click()
    await expect(this.page.getByTestId(TestIds.appShell)).toBeVisible({
      timeout: 60_000,
    })
  }

  async expectLoginVisible() {
    await expect(this.panel).toBeVisible()
  }

  async switchToRegister() {
    await this.page.getByTestId(TestIds.loginRegisterToggle).click()
  }

  async register(email: string, password: string, displayName: string) {
    await this.switchToRegister()
    await this.page.getByTestId(TestIds.loginDisplayNameInput).fill(displayName)
    await this.email.fill(email)
    await this.password.fill(password)
    await this.page.getByTestId(TestIds.loginSubmitButton).click()
  }
}
