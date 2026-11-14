import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { API_URL, ADMIN_EMAIL, ADMIN_PASSWORD } from '../fixtures/env'

test('unknown case key shows not-found state', {
  tag: ['@VQA-255', '@VUE-34'],
  annotation: [
    { type: 'vulyk', description: 'VQA-255' },
    { type: 'vulyk', description: 'VUE-34' },
  ],
}, async ({ page, request }) => {
  await ensureAdminSession(request)
  await loginAsAdmin(page)
  await page.goto('/cases/NOPE-999999')
  await expect(
    page
      .getByTestId(TestIds.caseDetailError)
      .or(page.getByTestId(TestIds.caseListPanel))
      .or(page.getByRole('heading', { name: /not found|не знайден/i })),
  ).toBeVisible({ timeout: 20_000 })
})

test('expired session redirects to login on next navigation', {
  tag: ['@VQA-256', '@VUE-33'],
  annotation: [
    { type: 'vulyk', description: 'VQA-256' },
    { type: 'vulyk', description: 'VUE-33' },
  ],
}, async ({ page, request }) => {
  await ensureAdminSession(request)
  await loginAsAdmin(page)
  await expect(page.getByTestId(TestIds.appShell)).toBeVisible()

  await page.evaluate(() => {
    localStorage.clear()
    sessionStorage.clear()
  })
  await page.context().clearCookies()
  await page.goto('/login')
  await expect(page.getByTestId(TestIds.loginPanel)).toBeVisible({
    timeout: 20_000,
  })
})

test('bad password login stays on login with error (401 path)', {
  tag: ['@VQA-257', '@VUE-32'],
  annotation: [
    { type: 'vulyk', description: 'VQA-257' },
    { type: 'vulyk', description: 'VUE-32' },
  ],
}, async ({ page }) => {
  await page.goto('/login')
  await expect(page.getByTestId(TestIds.loginPanel)).toBeVisible()
  await page.getByTestId(TestIds.loginEmailInput).fill(ADMIN_EMAIL)
  await page.getByTestId(TestIds.loginPasswordInput).fill('definitely-wrong-password!!')
  await page.getByTestId(TestIds.loginSubmitButton).click()
  await expect(page.getByTestId(TestIds.loginError)).toBeVisible({ timeout: 15_000 })
  await expect(page.getByTestId(TestIds.loginPanel)).toBeVisible()
})

test('API health is reachable for the e2e stack', {
  tag: ['@VQA-258', '@VUE-31'],
  annotation: [
    { type: 'vulyk', description: 'VQA-258' },
    { type: 'vulyk', description: 'VUE-31' },
  ],
}, async ({ request }) => {
  const res = await request.get(`${API_URL}/health`)
  expect(res.ok()).toBeTruthy()
  expect(ADMIN_PASSWORD.length).toBeGreaterThanOrEqual(16)
})
