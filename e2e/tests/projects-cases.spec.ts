import { expect, test } from '@playwright/test'
import {
  createCase,
  createProject,
  createSuite,
} from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { AdminPage } from '../pages/AdminPage'
import { CasesPage } from '../pages/CasesPage'

test.describe.configure({ mode: 'serial' })

let projectKey = ''
let projectId = ''
let adminToken = ''

test.beforeAll(async ({ request }) => {
  const admin = await ensureAdminSession(request)
  adminToken = admin.token
  projectKey = uniqueKey('PC')
  const project = await createProject(request, admin.token, {
    key: projectKey,
    name: `Cases ${projectKey}`,
  })
  projectId = project.id
  await createSuite(request, admin.token, projectId, 'Smoke')
})

test('admin can create a project via UI', async ({ page }) => {
  await loginAsAdmin(page)
  const admin = new AdminPage(page)
  await admin.open()
  const key = uniqueKey('PJ')
  await admin.createProject(key, `Project ${key}`)
})

test('suite tree, case CRUD with steps, search, filter, bulk', async ({
  page,
  request,
}) => {
  await loginAsAdmin(page)

  // Ensure our project is selected
  const select = page.locator('.project-select')
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })

  // Suite drawer / tree shows Smoke
  await expect(page.getByRole('button', { name: 'Smoke' })).toBeVisible({
    timeout: 20_000,
  })

  const cases = new CasesPage(page)
  await cases.open()

  const caseKey = `${projectKey}-C1`
  const title = `CRUD case ${caseKey}`
  await cases.createCase(caseKey, title)

  // Open detail and add a step
  await cases.openCaseByTitle(title)
  await page.getByRole('button', { name: 'Edit' }).click()
  await page.getByRole('button', { name: 'Add step' }).click()
  const step = page.locator('.step').last()
  await step.locator('textarea').nth(0).fill('Open app')
  await step.locator('textarea').nth(1).fill('Home loads')
  await page.getByRole('button', { name: /Save/i }).click()
  await expect(page.getByText('Open app')).toBeVisible({ timeout: 20_000 })

  await page.getByRole('link', { name: /Back to cases/i }).click()

  // Second case for bulk/filter
  const caseKey2 = `${projectKey}-C2`
  await cases.createCase(caseKey2, `Filter me ${caseKey2}`)

  await cases.search(caseKey)
  await expect(page.getByText(title)).toBeVisible()

  await cases.search('')
  await cases.filterStatus('draft')
  // Newly created cases are typically draft
  await expect(page.locator('.table-row').first()).toBeVisible({ timeout: 20_000 })

  await cases.filterStatus('')
  await cases.selectCase(caseKey2)
  await cases.bulkMarkReady()
  await expect(page.locator(`.status-pill.status-ready`).first()).toBeVisible({
    timeout: 20_000,
  })

  // API bulk still works for isolation
  const listed = await createCase(request, adminToken, projectId, {
    key: `${projectKey}-API`,
    title: `API case ${projectKey}`,
    priority: 'low',
    status: 'ready',
    steps: [],
  })
  expect(listed).toBeTruthy()
})
