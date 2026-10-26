import { expect, test } from '@playwright/test'
import {
  authStatus,
  listSsoProvidersAdmin,
  updateAdminSettings,
  updateSsoProvider,
} from '../fixtures/api'
import {
  ensureAdminSession,
  loginAsAdmin,
  logout,
} from '../fixtures/auth'
import {
  API_URL,
  OIDC_CLIENT_ID,
  OIDC_CLIENT_SECRET,
  OIDC_ISSUER,
  uniqueKey,
} from '../fixtures/env'
import { LoginPage } from '../pages/LoginPage'

test.describe.configure({ mode: 'serial' })

test('registration open flow creates an account', async ({ page, request }) => {
  await ensureAdminSession(request)
  const status = await authStatus(request)
  test.skip(
    status.registration_mode !== 'open',
    `registration_mode=${status.registration_mode}`,
  )

  await loginAsAdmin(page)
  await logout(page)

  const email = `reg-${uniqueKey('').toLowerCase()}@vulyk.local`
  const login = new LoginPage(page)
  await login.goto()
  await expect(
    page.getByRole('button', { name: /Create an account/i }),
  ).toBeVisible()
  await login.register(email, 'reg-pass-12345678', 'Registered User')
  await expect(page.locator('.app-shell')).toBeVisible({ timeout: 60_000 })
})

test('registration disabled hides create-account (when not forced open)', async ({
  page,
  request,
}) => {
  test.skip(
    process.env.VULYK_REGISTRATION_MODE === 'open',
    'VULYK_REGISTRATION_MODE=open forces open via env; disabled UI cannot be asserted',
  )

  const admin = await ensureAdminSession(request)
  await updateAdminSettings(request, admin.token, {
    registration_mode: 'disabled',
  })

  await loginAsAdmin(page)
  await logout(page)
  await expect(
    page.getByRole('button', { name: /Create an account/i }),
  ).toHaveCount(0)

  const denied = await request.post(`${API_URL}/api/v1/auth/register`, {
    data: {
      email: `disabled-${uniqueKey('').toLowerCase()}@vulyk.local`,
      password: 'disabled-pass-123',
      display_name: 'Nope',
    },
  })
  expect(denied.status()).toBe(403)

  // Restore open for subsequent suites when possible
  await updateAdminSettings(request, admin.token, {
    registration_mode: 'open',
  })
})

test('SSO via mock OIDC', async ({ page, request }) => {
  test.skip(
    process.env.E2E_SKIP_MOCK_OIDC === '1',
    'mock OIDC disabled',
  )

  const admin = await ensureAdminSession(request)
  const baseUrl = process.env.E2E_BASE_URL ?? 'http://127.0.0.1:5173'
  // SSO callback + cookie must round-trip through the UI origin (Vite proxy).
  await updateAdminSettings(request, admin.token, { base_url: baseUrl })

  const providers = await listSsoProvidersAdmin(request, admin.token)
  const oidc = providers.find((p) => p.provider_key === 'oidc')
  expect(oidc, 'seeded oidc provider row').toBeTruthy()

  await updateSsoProvider(request, admin.token, oidc!.id, {
    enabled: true,
    issuer_url: OIDC_ISSUER,
    client_id: OIDC_CLIENT_ID,
    client_secret: OIDC_CLIENT_SECRET,
    display_name: 'Mock OIDC',
    scopes: 'openid email profile',
  })

  // Confirm discovery is reachable
  const discovery = await request.get(
    `${OIDC_ISSUER}/.well-known/openid-configuration`,
  )
  expect(discovery.ok()).toBeTruthy()

  await loginAsAdmin(page)
  await logout(page)

  await expect(
    page.getByRole('button', { name: /Continue with Mock OIDC/i }),
  ).toBeVisible({ timeout: 20_000 })

  await page.getByRole('button', { name: /Continue with Mock OIDC/i }).click()
  // Mock authorize redirects back to API callback → UI session
  await expect(page.locator('.app-shell, .login-panel').first()).toBeVisible({
    timeout: 60_000,
  })
  // Successful SSO lands in the app
  await expect(page.locator('.app-shell')).toBeVisible({ timeout: 60_000 })
})
