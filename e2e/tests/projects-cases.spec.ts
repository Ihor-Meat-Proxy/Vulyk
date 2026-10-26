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
  await admin.createProject(`Project ${key}`, key)
})

test('admin can create a project with name only (auto key)', async ({
  page,
}) => {
  await loginAsAdmin(page)
  const admin = new AdminPage(page)
  await admin.open()
  const name = `Kyiv Auto Concert ${uniqueKey('AC')}`
  await admin.createProject(name)
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

  const cases = new CasesPage(page)
  await cases.open()

  // Folder tree panel is present (SquashTM-style); Smoke appears there (and may also be in the nav drawer)
  await expect(cases.folderTree()).toBeVisible({ timeout: 20_000 })
  await expect(cases.folderTree().getByText('Smoke')).toBeVisible({
    timeout: 20_000,
  })

  const title = `CRUD case ${projectKey}`
  await cases.createCase(title)

  // Auto-allocated key appears in the list (PREFIX-N)
  const keyBadge = page.locator('.table-row .badge').first()
  await expect(keyBadge).toBeVisible({ timeout: 20_000 })
  const allocatedKey = (await keyBadge.textContent())?.trim() ?? ''
  expect(allocatedKey).toMatch(/^[A-Z][A-Z0-9]+-\d+$/)

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
  const title2 = `Filter me ${projectKey}`
  await cases.createCase(title2)

  await cases.search(projectKey)
  await expect(page.getByText(title)).toBeVisible()

  await cases.search('')
  await cases.filterStatus('ready')
  await expect(page.locator('.table-row').first()).toBeVisible({ timeout: 20_000 })

  await cases.filterStatus('')
  const secondKey = (
    await page.locator('.table-row').filter({ hasText: title2 }).locator('.badge').textContent()
  )?.trim()
  expect(secondKey).toBeTruthy()
  await cases.selectCase(secondKey!)
  await cases.bulkMarkReady()
  await expect(page.locator(`.status-pill.status-ready`).first()).toBeVisible({
    timeout: 20_000,
  })

  // Nested folder create + filter
  await cases.createFolder(`Nested ${projectKey}`)
  await expect(cases.folderTree().getByText(`Nested ${projectKey}`)).toBeVisible({
    timeout: 20_000,
  })

  // API create still works; key is allocated server-side
  const listed = await createCase(request, adminToken, projectId, {
    title: `API case ${projectKey}`,
    priority: 'low',
    status: 'ready',
    steps: [],
  })
  expect(listed.case.key).toMatch(/^[A-Z][A-Z0-9]+-\d+$/)
})
