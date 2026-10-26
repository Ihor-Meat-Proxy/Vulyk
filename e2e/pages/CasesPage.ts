import { expect, type Page } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'

export class CasesPage {
  constructor(readonly page: Page) {}

  listPanel() {
    return this.page.getByTestId(TestIds.caseListPanel)
  }

  caseCount() {
    return this.listPanel().getByTestId(TestIds.caseListCount)
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

  columnsButton() {
    return this.page.getByTestId(TestIds.caseListColumnsButton)
  }

  columnsDialog() {
    return this.page.getByTestId(TestIds.caseListColumnsMenu)
  }

  async openColumnsSettings() {
    await this.columnsButton().click()
    await expect(this.columnsDialog()).toBeVisible()
  }

  caseListHead() {
    return this.page.getByTestId(TestIds.caseListHead)
  }

  columnRowById(columnId: string) {
    return this.columnsDialog().locator(
      `[data-testid="${TestIds.caseListColumnItem}"][data-entity-key="${columnId}"]`,
    )
  }

  async toggleColumn(columnId: string, checked: boolean) {
    const box = this.columnRowById(columnId).getByTestId(
      TestIds.caseListColumnItemCheckbox,
    )
    if (checked) await box.check()
    else await box.uncheck()
  }

  async moveColumn(columnId: string, direction: 'left' | 'right') {
    const row = this.columnRowById(columnId)
    const btn =
      direction === 'left'
        ? row.getByTestId(TestIds.caseListColumnItemMoveLeft)
        : row.getByTestId(TestIds.caseListColumnItemMoveRight)
    await btn.click()
  }

  async resetColumns() {
    await this.columnsDialog()
      .getByTestId(TestIds.caseListColumnsResetButton)
      .click()
  }

  async sortByColumn(label: RegExp | string) {
    await this.caseListHead()
      .getByRole('button', { name: new RegExp(String(label), 'i') })
      .click()
  }

  async exportSelectedPdf() {
    const downloadPromise = this.page.waitForEvent('download')
    await this.page.getByTestId(TestIds.caseListExportPdf).click()
    return downloadPromise
  }

  async exportRowPdf(key: string) {
    const downloadPromise = this.page.waitForEvent('download')
    await this.page
      .locator(
        `[data-testid="${TestIds.caseRowExportPdf}"][data-entity-key="${key}"]`,
      )
      .click()
    return downloadPromise
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

  exportDetailButton() {
    return this.page.getByTestId(TestIds.caseDetailExportPdf)
  }

  async exportDetailPdf() {
    const downloadPromise = this.page.waitForEvent('download')
    await this.exportDetailButton().click()
    return downloadPromise
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
    // window.prompt blocks the click until the dialog is handled — accept in parallel.
    await Promise.all([
      this.page.waitForEvent('dialog').then((dialog) => dialog.accept(name)),
      this.folderTree().getByTestId(TestIds.folderTreeNewButton).click(),
    ])
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
