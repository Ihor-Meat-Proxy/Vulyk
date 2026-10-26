import { expect, type Page } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'

export class CasesPage {
  constructor(readonly page: Page) {}

  listPanel() {
    return this.page.getByTestId(TestIds.caseListPanel)
  }

  async open() {
    await this.page.getByTestId(TestIds.navLinkCases).click()
  }

  async createCase(title: string) {
    await this.page.getByTestId(TestIds.caseListNewTitleInput).fill(title)
    await this.page.getByTestId(TestIds.caseListNewCaseButton).click()
    await expect(this.listPanel().getByText(title)).toBeVisible({
      timeout: 30_000,
    })
  }

  async search(query: string) {
    await this.page.getByTestId(TestIds.caseListSearchInput).fill(query)
  }

  async filterStatus(status: string) {
    await this.page
      .getByTestId(TestIds.caseListStatusFilter)
      .selectOption(status)
  }

  async selectCase(key: string) {
    await this.page
      .locator(
        `[data-testid="${TestIds.caseRow}"][data-entity-key="${key}"]`,
      )
      .getByTestId(TestIds.caseRowCheckbox)
      .check()
  }

  async bulkMarkReady() {
    await this.page.getByTestId(TestIds.caseListMarkReadyButton).click()
  }

  async openCaseByTitle(title: string) {
    await this.listPanel()
      .getByTestId(TestIds.caseRowTitle)
      .filter({ hasText: title })
      .click()
  }

  async backToCases() {
    await this.page.getByTestId('case-detail-back').click()
  }

  folderTree() {
    return this.page.getByTestId(TestIds.folderTree)
  }

  async selectFolder(name: string | RegExp) {
    await this.folderTree()
      .getByTestId(TestIds.folderNodeButton)
      .filter({ hasText: name })
      .first()
      .click()
  }

  async createFolder(name: string) {
    this.page.once('dialog', async (dialog) => {
      await dialog.accept(name)
    })
    await this.folderTree().getByTestId(TestIds.folderTreeNewButton).click()
  }

  async editSteps(action: string, expected: string) {
    await this.page.getByTestId(TestIds.caseDetailEditButton).click()
    const addStep = this.page.getByTestId(TestIds.caseDetailAddStepButton)
    if (await addStep.isVisible()) {
      await addStep.click()
    }
    const steps = this.page.getByTestId(TestIds.caseDetailStep)
    const last = steps.last()
    await last.getByTestId(TestIds.caseDetailStepAction).fill(action)
    await last.getByTestId(TestIds.caseDetailStepExpected).fill(expected)
    await this.page.getByTestId(TestIds.caseDetailSaveButton).click()
  }
}
