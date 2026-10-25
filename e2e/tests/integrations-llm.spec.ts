import { expect, test } from '@playwright/test'
import { createProject } from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { API_URL, uniqueKey } from '../fixtures/env'
import { AdminPage } from '../pages/AdminPage'

test.describe.configure({ mode: 'serial' })

test('integrations + LLM settings pages (mock LLM)', async ({
  page,
  request,
}) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('IL')
  await createProject(request, admin.token, {
    key,
    name: `Integrations ${key}`,
  })

  await loginAsAdmin(page)
  const select = page.locator('.project-select')
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(key))
  if (idx >= 0) await select.selectOption({ index: idx })

  const adminUi = new AdminPage(page)
  await adminUi.open()

  await adminUi.openIntegrationsTab()
  await expect(
    page.getByRole('heading', { name: 'Integrations', exact: true }),
  ).toBeVisible()

  await adminUi.openLlmTab()
  await expect(page.getByRole('heading', { name: /LLM providers/i })).toBeVisible()

  // Save a mock provider config
  const providerSelect = page.locator('form.stack-form').getByLabel(/^Provider$/i)
  if (await providerSelect.isVisible()) {
    const values = await providerSelect.locator('option').allTextContents()
    const mockLabel = values.find((v) => /mock/i.test(v))
    if (mockLabel) {
      await providerSelect.selectOption({ label: mockLabel })
    }
    await page.locator('form.stack-form').getByLabel(/^Name$/i).fill(`Mock ${key}`)
    await page.locator('form.stack-form').getByLabel(/^Model$/i).fill('mock-1')
    await page.getByRole('button', { name: /Save provider|Save/i }).click()
    await expect(page.getByText(`Mock ${key}`)).toBeVisible({ timeout: 20_000 })
  }

  // Providers API lists mock when VULYK_LLM_MOCK=1
  const providers = await request.get(`${API_URL}/api/v1/llm/providers`, {
    headers: { Authorization: `Bearer ${admin.token}` },
  })
  expect(providers.ok()).toBeTruthy()
  const list = await providers.json()
  expect(Array.isArray(list)).toBeTruthy()
  expect(list.some((p: { id: string }) => p.id === 'mock' || /mock/i.test(p.id))).toBeTruthy()
})

test('CI trigger config page', async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('CI')
  await createProject(request, admin.token, { key, name: `CI ${key}` })

  await loginAsAdmin(page)
  const select = page.locator('.project-select')
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(key))
  if (idx >= 0) await select.selectOption({ index: idx })

  await page
    .getByRole('navigation', { name: 'Primary' })
    .getByRole('link', { name: /^CI$/i })
    .click()
  await expect(page.getByRole('heading', { name: 'CI triggers' })).toBeVisible()
  await expect(page.getByLabel('CI provider')).toBeVisible()
  await expect(page.getByLabel('CI config name')).toBeVisible()
  await expect(page.getByLabel('CI config JSON')).toBeVisible()

  await page.getByLabel('CI config name').fill(`GH ${key}`)
  await page.getByRole('button', { name: 'Save config' }).click()
  await expect(page.getByText(`GH ${key}`)).toBeVisible({ timeout: 20_000 })
})
