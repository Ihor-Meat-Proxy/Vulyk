import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import { createUser } from '../fixtures/api'
import {
  ensureAdminSession,
  logout,
  loginAsAdmin,
} from '../fixtures/auth'
import { API_URL, uniqueKey } from '../fixtures/env'
import { LoginPage } from '../pages/LoginPage'

test.describe.configure({ mode: 'serial' })

const STRONG = `LockoutPass-${uniqueKey('pw').toLowerCase()}9a`

test('auth status exposes password policy min length', {
  tag: '@VQA-277', annotation: { type: 'vulyk', description: 'VQA-277' },
}, async ({ request }) => {
  const res = await request.get(`${API_URL}/api/v1/auth/status`)
  expect(res.ok()).toBeTruthy()
  const body = (await res.json()) as {
    password_policy?: {
      min_length?: number
      require_letter?: boolean
      require_digit?: boolean
    }
  }
  expect(body.password_policy?.min_length).toBeGreaterThanOrEqual(12)
  expect(body.password_policy?.require_letter).toBe(true)
  expect(body.password_policy?.require_digit).toBe(true)
})

test('registration rejects weak password below policy', {
  tag: '@VQA-278', annotation: { type: 'vulyk', description: 'VQA-278' },
}, async ({
  page,
  request,
}) => {
  await loginAsAdmin(page)
  await logout(page)

  const login = new LoginPage(page)
  await login.goto()
  await expect(page.getByTestId(TestIds.loginPanel)).toBeVisible()
  await expect(page.getByTestId(TestIds.loginRegisterToggle)).toBeVisible()

  const email = `weak-${uniqueKey('').toLowerCase()}@vulyk.local`
  await login.switchToRegister()
  await page.getByTestId(TestIds.loginDisplayNameInput).fill('Weak User')
  await page.getByTestId(TestIds.loginEmailInput).fill(email)
  await page.getByTestId(TestIds.loginPasswordInput).fill('short1')
  await expect(page.getByTestId(TestIds.passwordRequirements)).toBeVisible()
  // Client-side: submit stays disabled until the password meets policy.
  await expect(page.getByTestId(TestIds.loginSubmitButton)).toBeDisabled()
  await expect(page.getByTestId(TestIds.appShell)).toHaveCount(0)

  const apiReject = await request.post(`${API_URL}/api/v1/auth/register`, {
    data: {
      email: `weak-api-${uniqueKey('').toLowerCase()}@vulyk.local`,
      password: 'short1',
      display_name: 'Weak API',
    },
  })
  expect(apiReject.status()).toBeGreaterThanOrEqual(400)
  expect(apiReject.status()).toBeLessThan(500)
})

test('repeated failed logins lock out then recover', {
  tag: '@VQA-279', annotation: { type: 'vulyk', description: 'VQA-279' },
}, async ({
  page,
  request,
}) => {
  const admin = await ensureAdminSession(request)
  const email = `lock-${uniqueKey('').toLowerCase()}@vulyk.local`
  expect(STRONG.length).toBeGreaterThanOrEqual(16)
  await createUser(request, admin.token, {
    email,
    password: STRONG,
    display_name: 'Lockout Fixture',
  })

  await loginAsAdmin(page)
  await logout(page)

  const login = new LoginPage(page)
  await login.goto()
  await expect(page.getByTestId(TestIds.loginPanel)).toBeVisible()
  expect(
    await page.getByTestId(TestIds.loginPasswordInput).getAttribute('minlength'),
  ).toBeNull()

  // Drive lockout through the API for deterministic 429 (same throttle as UI).
  let saw429 = false
  for (let i = 0; i < 8; i++) {
    const res = await request.post(`${API_URL}/api/v1/auth/login`, {
      data: { email, password: 'definitely-wrong-password-xx' },
    })
    if (res.status() === 429) {
      saw429 = true
      break
    }
    expect([401, 403]).toContain(res.status())
  }
  expect(saw429).toBeTruthy()

  await page.getByTestId(TestIds.loginEmailInput).fill(email)
  await page.getByTestId(TestIds.loginPasswordInput).fill('still-wrong-password')
  await page.getByTestId(TestIds.loginSubmitButton).click()
  await expect(page.getByTestId(TestIds.loginError)).toBeVisible({
    timeout: 15_000,
  })
  await expect(page.getByTestId(TestIds.appShell)).toHaveCount(0)

  // Initial lock is ~2s; poll correct login until throttle expires.
  await expect
    .poll(
      async () => {
        const res = await request.post(`${API_URL}/api/v1/auth/login`, {
          data: { email, password: STRONG },
        })
        return res.status()
      },
      { timeout: 30_000, intervals: [500, 1000, 2000] },
    )
    .toBe(200)

  await page.getByTestId(TestIds.loginEmailInput).fill(email)
  await page.getByTestId(TestIds.loginPasswordInput).fill(STRONG)
  await page.getByTestId(TestIds.loginSubmitButton).click()
  await expect(page.getByTestId(TestIds.appShell)).toBeVisible({
    timeout: 60_000,
  })
})
