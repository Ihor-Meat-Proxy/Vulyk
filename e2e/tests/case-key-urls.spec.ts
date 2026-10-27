import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import {
  createCase,
  createProject,
  createSuite,
  deleteCase,
} from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { CasesPage } from '../pages/CasesPage'

test.describe.configure({ mode: 'serial' })

let projectKey = ''
let projectId = ''
let adminToken = ''
let suiteId = ''
let caseKey = ''
let caseId = ''
let caseTitle = ''

test.beforeAll(async ({ request }) => {
  const admin = await ensureAdminSession(request)
  adminToken = admin.token
  projectKey = uniqueKey('CK')
  const project = await createProject(request, admin.token, {
    key: projectKey,
    name: `Case Keys ${projectKey}`,
  })
  projectId = project.id
  const suite = await createSuite(request, admin.token, projectId, 'Folder A')
  suiteId = suite.id
  caseTitle = `Key URL case ${projectKey}`
  const created = await createCase(request, admin.token, projectId, {
    title: caseTitle,
    suite_id: suiteId,
  })
  caseId = created.case.id
  caseKey = created.case.key
})

async function selectProject(page: import('@playwright/test').Page) {
  const select = page.getByTestId(TestIds.headerProjectSelect)
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })
}

test('case detail uses key URL; UUID redirects; copy + breadcrumbs + backspace', async ({
  page,
  context,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']).catch(() => {
    /* some browsers reject clipboard-read; write still works in Chromium */
  })
  await loginAsAdmin(page)
  await selectProject(page)

  const cases = new CasesPage(page)
  await cases.open()
  await cases.selectFolder('Folder A')
  await cases.openCaseByTitle(caseTitle)

  await expect(page).toHaveURL(new RegExp(`/cases/${caseKey}$`))
  await expect(page.getByTestId(TestIds.caseDetailPage)).toBeVisible()
  await expect(page.getByTestId(TestIds.caseDetailBreadcrumbs)).toBeVisible()
  await expect(page.getByTestId(TestIds.caseDetailBreadcrumbCurrent)).toContainText(
    caseKey,
  )
  await expect(page.getByTestId(TestIds.caseDetailBackLink)).toBeVisible()

  // Copy key badge → toast
  await page.getByTestId(TestIds.caseDetailKeyBadge).click()
  await expect(page.getByTestId(TestIds.appToast)).toContainText(caseKey, {
    timeout: 5_000,
  })

  // UUID URL redirects (replace) to key URL
  await page.goto(`/cases/${caseId}`)
  await expect(page).toHaveURL(new RegExp(`/cases/${caseKey}$`), {
    timeout: 20_000,
  })
  await expect(page.getByTestId(TestIds.caseDetailPage)).toBeVisible({
    timeout: 20_000,
  })

  // Backspace returns to folder-filtered list (focus outside inputs)
  await page.getByTestId(TestIds.caseDetailPage).click()
  await page.keyboard.press('Backspace')
  await expect(page).toHaveURL(/\/(?:\?.*)?$/)
  await expect(
    page.locator(
      `[data-testid="${TestIds.caseRow}"]`,
    ).filter({ has: page.getByTestId(TestIds.caseRowTitle) }),
  ).toBeVisible({ timeout: 20_000 })
})

test('deleted case key shows case deleted page', async ({ page, request }) => {
  const doomedTitle = `Doomed ${projectKey}`
  const created = await createCase(request, adminToken, projectId, {
    title: doomedTitle,
    suite_id: suiteId,
  })
  const doomedKey = created.case.key as string
  const doomedId = created.case.id as string
  await deleteCase(request, adminToken, projectId, doomedId)

  await loginAsAdmin(page)
  await selectProject(page)
  await page.goto(`/cases/${doomedKey}`)
  await expect(page.getByTestId(TestIds.caseDetailDeleted)).toBeVisible({
    timeout: 20_000,
  })
  await expect(page.getByTestId(TestIds.caseDetailDeleted)).not.toHaveText('')
})
