import { expect, test } from '@playwright/test'
import { createProject, createSuite } from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { API_URL, uniqueKey } from '../fixtures/env'
import { CasesPage } from '../pages/CasesPage'

test.describe.configure({ mode: 'serial' })

test('chat propose preview, change folder, confirm create', async ({
  page,
  request,
}) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('CH')
  const project = await createProject(request, admin.token, {
    key,
    name: `Chat Propose ${key}`,
  })
  await createSuite(request, admin.token, project.id, 'Auth')

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

  await page
    .getByRole('navigation', { name: 'Primary' })
    .getByRole('link', { name: /^Chat$|Чат/i })
    .click()

  await expect(
    page.getByRole('heading', { name: /Chat with your tests|Чат із вашими тестами/i }),
  ).toBeVisible()

  // Model picker reused from Generate.
  const model = page.getByLabel(/^Model$|^Модель$/i)
  await expect(model).toBeVisible()
  await expect(model.locator('option')).toContainText([/Mock/i])

  await page
    .getByPlaceholder(/Ask about this project|Запитайте про тести/i)
    .fill('Create a test case for password reset via email')
  await page.getByRole('button', { name: /^Ask$|^Запитати$/i }).click()

  const card = page.getByRole('region', { name: /Proposed creation|Пропоноване створення/i })
  await expect(card).toBeVisible({ timeout: 20_000 })
  await expect(card).toContainText(/right project and folder|правильний проєкт/i)

  // Change folder to Auth suite.
  await card.getByLabel(/^Folder$|^Тека$/i).selectOption({ label: 'Auth' })

  // Inline edit title (first draft input under Case heading).
  const titleInput = card.locator('.chat-draft-item input').first()
  await titleInput.fill(`Password reset ${key}`)

  await card.getByRole('button', { name: /^Create$|^Створити$/i }).click()

  await expect(page.locator('.notice')).toContainText(/Created|Створено/i, {
    timeout: 20_000,
  })
  await expect(card).toHaveCount(0)

  // Case appears in Cases list with auto key.
  await page
    .getByRole('navigation', { name: 'Primary' })
    .getByRole('link', { name: /Cases|Кейси/i })
    .click()
  await expect(page.getByText(`Password reset ${key}`)).toBeVisible({
    timeout: 30_000,
  })
})

test('chat clarify when folder is ambiguous', async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('CL')
  const project = await createProject(request, admin.token, {
    key,
    name: `Chat Clarify ${key}`,
  })
  await createSuite(request, admin.token, project.id, 'Auth')
  await createSuite(request, admin.token, project.id, 'Billing')

  await request.post(`${API_URL}/api/v1/projects/${project.id}/llm/configs`, {
    headers: { Authorization: `Bearer ${admin.token}` },
    data: {
      provider: 'mock',
      name: `Mock ${key}`,
      model: 'mock-1',
      is_default: true,
    },
  })

  await loginAsAdmin(page)
  const select = page.locator('.project-select')
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(key))
  if (idx >= 0) await select.selectOption({ index: idx })

  await page
    .getByRole('navigation', { name: 'Primary' })
    .getByRole('link', { name: /^Chat$|Чат/i })
    .click()

  await page
    .getByPlaceholder(/Ask about this project|Запитайте про тести/i)
    .fill('Create cases — which folder or Auth folder should I use?')
  await page.getByRole('button', { name: /^Ask$|^Запитати$/i }).click()

  await expect(page.locator('.chat-bubble.assistant').last()).toContainText(
    /Which folder|яку теку|folder/i,
    { timeout: 20_000 },
  )
  await expect(page.getByRole('region', { name: /Proposed creation|Пропоноване створення/i })).toHaveCount(0)
})
