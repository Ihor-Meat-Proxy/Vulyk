import { expect, test } from '@playwright/test'
import { createCase, createProject } from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { CasesPage } from '../pages/CasesPage'
import { TestIds } from '../../frontend/src/testing/testIds'

test.describe.configure({ mode: 'serial' })

let projectKey = ''
let projectId = ''
let adminToken = ''
let caseTitle = ''

test.beforeAll(async ({ request }) => {
  const admin = await ensureAdminSession(request)
  adminToken = admin.token
  projectKey = uniqueKey('KE')
  const project = await createProject(request, admin.token, {
    key: projectKey,
    name: `Keyboard ${projectKey}`,
  })
  projectId = project.id
  caseTitle = `Keyboard case ${projectKey}`
  await createCase(request, adminToken, projectId, {
    title: caseTitle,
    steps: [
      { action: 'Open login', expected_result: 'Form visible', data: '' },
    ],
  })
})

async function selectProject(page: import('@playwright/test').Page) {
  const select = page.getByTestId(TestIds.headerProjectSelect)
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })
}

async function openCaseEditor(page: import('@playwright/test').Page) {
  await loginAsAdmin(page)
  await selectProject(page)
  const cases = new CasesPage(page)
  await cases.open()
  await cases.search(caseTitle)
  await cases.openCaseByTitle(caseTitle)
  await page.getByTestId(TestIds.caseDetailEditButton).click()
  await expect(page.getByTestId(TestIds.caseDetailStickyBar)).toBeVisible()
}

test('Tab in last Expected with content adds a step and focuses Action', async ({
  page,
}) => {
  await openCaseEditor(page)

  const steps = page.getByTestId(TestIds.caseDetailStep)
  const lastExpected = steps.last().getByTestId(TestIds.caseDetailStepExpected)
  await lastExpected.click()
  await expect(page.getByTestId(TestIds.caseDetailStepHint)).toBeVisible()
  await lastExpected.press('Tab')

  const newAction = steps.last().getByTestId(TestIds.caseDetailStepAction)
  await expect(newAction).toBeFocused({ timeout: 10_000 })
  await expect(newAction).toHaveValue('')

  // Empty last step: Tab must not spawn another
  const before = await steps.count()
  await steps.last().getByTestId(TestIds.caseDetailStepExpected).click()
  await steps.last().getByTestId(TestIds.caseDetailStepExpected).press('Tab')
  await expect(steps).toHaveCount(before)

  // Shift+Tab keeps normal behaviour (no new step)
  await steps.last().getByTestId(TestIds.caseDetailStepExpected).press('Shift+Tab')
  await expect(steps).toHaveCount(before)
})

test('Ctrl/Cmd+S saves from textarea and shows toast; no changes toast', async ({
  page,
}) => {
  await openCaseEditor(page)

  const action = page
    .getByTestId(TestIds.caseDetailStep)
    .first()
    .getByTestId(TestIds.caseDetailStepAction)
  await action.fill(`Saved via shortcut ${projectKey}`)
  await expect(page.getByTestId(TestIds.caseDetailStickyUnsaved)).toBeVisible()

  const isMac = process.platform === 'darwin'
  await action.click()
  await page.keyboard.press(isMac ? 'Meta+s' : 'Control+s')

  await expect(
    page.getByTestId(TestIds.appToast).filter({ hasText: /saved|збереж/i }),
  ).toBeVisible({ timeout: 20_000 })
  await expect(page.getByTestId(TestIds.caseDetailEditButton)).toBeVisible({
    timeout: 20_000,
  })

  await page.getByTestId(TestIds.caseDetailEditButton).click()
  await page
    .getByTestId(TestIds.caseDetailStep)
    .first()
    .getByTestId(TestIds.caseDetailStepAction)
    .click()
  await page.keyboard.press(isMac ? 'Meta+s' : 'Control+s')
  await expect(
    page
      .getByTestId(TestIds.appToast)
      .filter({ hasText: /no changes|немає змін|sin cambios|aucun|keine/i }),
  ).toBeVisible({ timeout: 10_000 })
})

test('sticky Save/Cancel bar remains visible while scrolling', async ({
  page,
}) => {
  await openCaseEditor(page)

  await page.getByTestId(TestIds.caseDetailAddStepButton).click()
  await page.getByTestId(TestIds.caseDetailAddStepButton).click()
  await page.getByTestId(TestIds.caseDetailAddStepButton).click()
  await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight))

  const sticky = page.getByTestId(TestIds.caseDetailStickyBar)
  await expect(sticky).toBeVisible()
  const box = await sticky.boundingBox()
  expect(box).toBeTruthy()
  const viewport = page.viewportSize()
  expect(viewport).toBeTruthy()
  expect(box!.y + box!.height).toBeLessThanOrEqual((viewport!.height ?? 0) + 2)

  await page
    .getByTestId(TestIds.caseDetailStep)
    .first()
    .getByTestId(TestIds.caseDetailStepAction)
    .fill('dirty for sticky')
  await expect(page.getByTestId(TestIds.caseDetailStickyUnsaved)).toBeVisible()
})

test('Ctrl/Cmd+S does not preventDefault outside the case editor', async ({
  page,
}) => {
  await loginAsAdmin(page)
  await selectProject(page)
  const cases = new CasesPage(page)
  await cases.open()

  const prevented = await page.evaluate(() => {
    return new Promise<boolean>((resolve) => {
      const handler = (e: KeyboardEvent) => {
        resolve(e.defaultPrevented)
        window.removeEventListener('keydown', handler, true)
      }
      window.addEventListener('keydown', handler, true)
      window.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 's',
          ctrlKey: true,
          metaKey: false,
          bubbles: true,
          cancelable: true,
        }),
      )
      window.setTimeout(() => resolve(false), 50)
    })
  })
  expect(prevented).toBe(false)
})
