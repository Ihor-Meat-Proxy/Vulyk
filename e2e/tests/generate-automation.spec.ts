import { expect, test } from '@playwright/test'
import {
  createCase,
  createProject,
  createSuite,
} from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { API_URL, uniqueKey } from '../fixtures/env'
import { CasesPage } from '../pages/CasesPage'
import { GeneratePage } from '../pages/GeneratePage'

test.describe.configure({ mode: 'serial' })

test('Generate page: model dropdown, case picker, stubbed LLM code', async ({
  page,
  request,
}) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('GA')
  const project = await createProject(request, admin.token, {
    key,
    name: `Generate Auto ${key}`,
  })
  const suite = await createSuite(
    request,
    admin.token,
    project.id,
    'Smoke',
  )
  const created = await createCase(request, admin.token, project.id, {
    title: `Login coverage ${key}`,
    suite_id: suite.id,
  })

  // Stubbed mock LLM provider (no network).
  const cfg = await request.post(
    `${API_URL}/api/v1/projects/${project.id}/llm/configs`,
    {
      headers: { Authorization: `Bearer ${admin.token}` },
      data: {
        provider: 'mock',
        name: `Mock ${key}`,
        model: 'mock-1',
        is_default: true,
      },
    },
  )
  expect(cfg.ok()).toBeTruthy()

  await loginAsAdmin(page)
  const select = page.locator('.project-select')
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(key))
  if (idx >= 0) await select.selectOption({ index: idx })

  const gen = new GeneratePage(page)
  await gen.open()

  // Model dropdown lists the configured provider.
  const model = page.locator('form.stack-form').getByLabel(/^Model$|^Модель$/i).first()
  await expect(model).toBeVisible()
  await expect(model.locator('option')).toContainText([/Mock/i])

  await gen.generateDrafts('Users can reset passwords via email link')
  await expect(page.locator('.notice')).toContainText(/mock/i, {
    timeout: 20_000,
  })

  // Case picker: search + checkbox (chips show case key).
  const caseKey =
    (created as { key?: string }).key ??
    (created as { case?: { key?: string } }).case?.key ??
    key
  await page
    .getByPlaceholder(/Search by key or title|Пошук за ключем/i)
    .fill(caseKey)
  await gen.pickFirstCase()
  await expect(page.locator('.case-picker-chips .badge').first()).toBeVisible()

  await gen.generateCode()
  await expect(page.locator('.notice')).toContainText(/mock/i, {
    timeout: 20_000,
  })
  await expect(page.locator('pre.code-block')).toBeVisible()
  await expect(
    page.getByRole('button', { name: /Download code|Завантажити код/i }),
  ).toBeVisible()
})

test('Cases page bulk action opens Generate with selection', async ({
  page,
  request,
}) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('GB')
  const project = await createProject(request, admin.token, {
    key,
    name: `Bulk Gen ${key}`,
  })
  const suite = await createSuite(request, admin.token, project.id, 'Smoke')
  await createCase(request, admin.token, project.id, {
    title: `Bulk case ${key}`,
    suite_id: suite.id,
  })

  await loginAsAdmin(page)
  const select = page.locator('.project-select')
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(key))
  if (idx >= 0) await select.selectOption({ index: idx })

  const cases = new CasesPage(page)
  await cases.open()
  await cases.selectCase(`Bulk case ${key}`)
  await page
    .getByRole('button', {
      name: /Generate automation|Згенерувати автоматизацію/i,
    })
    .click()
  await expect(
    page.getByRole('heading', { name: /^Generate$|^Генерація$/i }),
  ).toBeVisible({ timeout: 20_000 })
  await expect(page.locator('.case-picker-chips .badge').first()).toBeVisible({
    timeout: 20_000,
  })
})

test('Admin Integrations shows Automation repo controls', async ({
  page,
  request,
}) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('AR')
  await createProject(request, admin.token, {
    key,
    name: `AutoRepo ${key}`,
  })

  await loginAsAdmin(page)
  const select = page.locator('.project-select')
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(key))
  if (idx >= 0) await select.selectOption({ index: idx })

  await page
    .getByRole('navigation', { name: 'Primary' })
    .getByRole('link', { name: /Admin|Адмін/i })
    .click()
  await page.getByRole('tab', { name: /Integrations|Інтеграції/i }).click()
  await expect(
    page.getByRole('heading', {
      name: /Automation repo|Репозиторій автоматизації/i,
    }),
  ).toBeVisible()
  await expect(
    page.getByRole('button', {
      name: /Test connection|Перевірити з/i,
    }),
  ).toBeVisible()
  await expect(
    page.getByRole('button', {
      name: /Re-index|Переіндексувати/i,
    }),
  ).toBeVisible()
})
