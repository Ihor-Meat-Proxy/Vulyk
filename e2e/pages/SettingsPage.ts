import { expect, type Page } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import { openUserMenu } from '../fixtures/header'

export class SettingsPage {
  constructor(readonly page: Page) {}

  async open() {
    await openUserMenu(this.page)
    await this.page.getByTestId(TestIds.headerGearSettings).click()
    await expect(this.page.getByTestId(TestIds.settingsPage)).toBeVisible({
      timeout: 15_000,
    })
  }

  async expectAutomationHeading() {
    await expect(
      this.page.getByTestId(TestIds.settingsAutomationPanel),
    ).toBeVisible({ timeout: 15_000 })
  }

  async expectCiTokensHeading() {
    await expect(this.page.getByTestId(TestIds.settingsCiPanel)).toBeVisible()
  }

  async fillNewRepo(opts: {
    name: string
    remoteUrl: string
    projectId?: string
  }) {
    await this.page.getByTestId(TestIds.settingsAutomationNameInput).fill(opts.name)
    await this.page
      .getByTestId(TestIds.settingsAutomationUrlInput)
      .fill(opts.remoteUrl)
    if (opts.projectId) {
      const box = this.page.locator(
        `[data-testid="${TestIds.settingsAutomationProjectCheckbox}"][data-entity-id="${opts.projectId}"]`,
      )
      if (await box.count()) {
        await box.check()
      }
    }
  }

  async submitAddRepo() {
    await this.page.getByTestId(TestIds.settingsAutomationSaveButton).click()
  }

  repoRows() {
    return this.page.getByTestId(TestIds.settingsAutomationRepoRow)
  }

  async selectCiProject(projectId: string) {
    await this.page
      .getByTestId(TestIds.settingsCiProjectSelect)
      .selectOption({ value: projectId })
  }

  async createCiToken(name: string) {
    await this.page.getByTestId(TestIds.settingsCiTokenNameInput).fill(name)
    await this.page.getByTestId(TestIds.settingsCiTokenCreateButton).click()
  }

  ciTokenSecret() {
    return this.page.getByTestId(TestIds.settingsCiTokenSecret)
  }

  ciTokenRows() {
    return this.page.getByTestId(TestIds.settingsCiTokenRow)
  }
}
