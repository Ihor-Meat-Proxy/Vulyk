import { expect, test } from '@playwright/test'
import { createProject } from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { API_URL, uniqueKey } from '../fixtures/env'
import { AdminPage } from '../pages/AdminPage'

test.describe.configure({ mode: 'serial' })

test('chat shows provider/model; project key optional; global default resolves', async ({
  page,
  request,
}) => {
  const admin = await ensureAdminSession(request)

  // Name-only create (no manual key) — mirrors CreateProject OpenAPI change.
  const concertName = `Kyiv Concert Chat ${uniqueKey('KC')}`
  const project = await createProject(request, admin.token, {
    name: concertName,
  })
  expect(project.key).toMatch(/^[A-Z][A-Z0-9_]{1,31}$/)
  expect(project.key.length).toBeGreaterThanOrEqual(2)

  // Save a project-scoped mock as default (real OpenRouter would be the same path).
  const cfg = await request.post(
    `${API_URL}/api/v1/projects/${project.id}/llm/configs`,
    {
      headers: { Authorization: `Bearer ${admin.token}` },
      data: {
        provider: 'mock',
        name: 'OR',
        model: 'mock-1',
        is_default: true,
        global: false,
      },
    },
  )
  expect(cfg.ok()).toBeTruthy()

  await loginAsAdmin(page)
  const select = page.locator('.project-select')
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(project.key) || o.includes(concertName))
  if (idx >= 0) await select.selectOption({ index: idx })

  await page
    .getByRole('navigation', { name: 'Primary' })
    .getByRole('link', { name: /^Chat$/i })
    .click()
  await expect(page.getByRole('heading', { name: /Chat with your tests/i })).toBeVisible()

  await page.getByPlaceholder(/Ask about this project/i).fill('Which cases cover login?')
  await page.getByRole('button', { name: /^Ask$/i }).click()

  const meta = page.locator('.chat-meta').last()
  await expect(meta).toBeVisible({ timeout: 20_000 })
  await expect(meta).toContainText(/Answered by mock\s*\/\s*mock-1/i)

  // Admin UI: optional key + global LLM checkbox present.
  const adminUi = new AdminPage(page)
  await adminUi.open()
  await adminUi.openTab(/Projects|Проєкти/i)
  await expect(
    page.locator('form.stack-form').getByLabel(/^Key \(optional\)$/i),
  ).toBeVisible()
  await expect(
    page.locator('form.stack-form').getByLabel(/^Name$/i),
  ).toBeVisible()

  await adminUi.openLlmTab()
  await expect(page.getByText(/Resolution: project provider/i)).toBeVisible()
  await expect(page.getByLabel(/Global \(all projects\)/i)).toBeVisible()
})

test('chat surfaces provider errors instead of silent mock', async ({
  request,
}) => {
  const admin = await ensureAdminSession(request)
  const project = await createProject(request, admin.token, {
    name: `Error Surface ${uniqueKey('ES')}`,
  })

  // Point OpenRouter-compatible config at a closed local port → clear HTTP error.
  const save = await request.post(
    `${API_URL}/api/v1/projects/${project.id}/llm/configs`,
    {
      headers: { Authorization: `Bearer ${admin.token}` },
      data: {
        provider: 'openai_compatible',
        name: 'Broken',
        model: 'broken-model',
        base_url: 'http://127.0.0.1:9/v1',
        api_key: 'sk-test',
        is_default: true,
      },
    },
  )
  expect(save.ok()).toBeTruthy()

  const ask = await request.post(
    `${API_URL}/api/v1/projects/${project.id}/llm/ask`,
    {
      headers: { Authorization: `Bearer ${admin.token}` },
      data: { question: 'ping' },
    },
  )
  expect(ask.ok()).toBeFalsy()
  const body = await ask.json()
  const message = String(body?.error?.message ?? '')
  expect(message).toMatch(/openai_compatible|broken-model|LLM call failed/i)
  expect(message).not.toMatch(/mock-1/)
})
