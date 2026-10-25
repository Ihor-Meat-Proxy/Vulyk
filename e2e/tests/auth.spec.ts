import { expect, test } from '@playwright/test'
import {
  ADMIN_EMAIL,
  ADMIN_PASSWORD,
  ensureAdminSession,
  loginAsAdmin,
  logout,
} from '../fixtures/auth'
import { LoginPage } from '../pages/LoginPage'

test.describe.configure({ mode: 'serial' })

test.beforeAll(async ({ request }) => {
  await ensureAdminSession(request)
})

test('login with seeded admin succeeds', async ({ page }) => {
  await loginAsAdmin(page)
  await expect(page.locator('.app-shell')).toBeVisible()
  await expect(page.getByRole('navigation', { name: 'Primary' })).toContainText(
    /Cases|Кейси/,
  )
})

test('bad credentials show an error', async ({ page }) => {
  const login = new LoginPage(page)
  await login.goto()
  if (await page.locator('.app-shell').isVisible()) {
    await logout(page)
  }
  await login.email.fill(ADMIN_EMAIL)
  await login.password.fill('definitely-wrong-password')
  await login.submit.click()
  await expect(login.error).toBeVisible({ timeout: 15_000 })
  await expect(page.locator('.app-shell')).toHaveCount(0)
})

test('logout returns to login panel', async ({ page }) => {
  await loginAsAdmin(page)
  await logout(page)
  await expect(page.getByLabel(/^Email$/i)).toHaveValue(/.*/)
  // Can sign back in
  await page.getByLabel(/^Email$/i).fill(ADMIN_EMAIL)
  await page.getByLabel(/Password|Пароль/i).fill(ADMIN_PASSWORD)
  await page.getByRole('button', { name: /Sign in|Увійти/i }).click()
  await expect(page.locator('.app-shell')).toBeVisible({ timeout: 60_000 })
})
