import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import { ensureAdminSession, logout, runSql } from '../fixtures/auth'
import { API_URL, uniqueKey } from '../fixtures/env'
import { LoginPage } from '../pages/LoginPage'

test.describe.configure({ mode: 'serial' })

/** Argon2id hash of plaintext `legacy1` (verified against vulyk-api). */
const LEGACY1_HASH =
  '$argon2id$v=19$m=19456,t=2,p=1$XXJTxQ8swVyOTdQcKqkRYg$sisW5ppIbTWToTQHB7r7lj46xWz4rKFNOTC0GfWrcnQ'

test('short legacy password signs in then forces 16+ change', async ({
  page,
  request,
}) => {
  test.skip(
    !process.env.DATABASE_URL,
    'DATABASE_URL required to insert a legacy short-password user',
  )

  await ensureAdminSession(request)

  const email = `legacy-${uniqueKey('').toLowerCase()}@vulyk.local`
  const weak = 'legacy1'
  const strong = `ChangedPass-${uniqueKey('pw').toLowerCase()}1`
  expect(strong.length).toBeGreaterThanOrEqual(16)

  // Escape single quotes for SQL literal.
  const hashSql = LEGACY1_HASH.replace(/'/g, "''")
  runSql(`
    INSERT INTO users (
      email, display_name, password_hash, is_system_admin, must_change_password
    ) VALUES (
      '${email}',
      'Legacy User',
      '${hashSql}',
      FALSE,
      FALSE
    );
  `)

  const login = new LoginPage(page)
  await login.goto()
  if (await page.getByTestId(TestIds.appShell).isVisible()) {
    await logout(page)
  }

  // Login must accept the short password (no HTML minLength on login).
  await page.getByTestId(TestIds.loginEmailInput).fill(email)
  await page.getByTestId(TestIds.loginPasswordInput).fill(weak)
  expect(
    await page.getByTestId(TestIds.loginPasswordInput).getAttribute('minlength'),
  ).toBeNull()
  await page.getByTestId(TestIds.loginSubmitButton).click()

  await expect(page.getByTestId(TestIds.changePasswordPanel)).toBeVisible({
    timeout: 30_000,
  })
  await expect(page.getByTestId(TestIds.changePasswordSubtitle)).toContainText(
    /no longer meets|security policy|політик|politique|política|Richtlinie/i,
  )

  const token = await page.evaluate(() => localStorage.getItem('vulyk.authToken'))
  expect(token).toBeTruthy()
  const blocked = await request.get(`${API_URL}/api/v1/projects`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  expect(blocked.status()).toBe(403)

  await page.getByTestId(TestIds.changePasswordNewInput).fill(strong)
  await page.getByTestId(TestIds.changePasswordConfirmInput).fill(strong)
  await expect(page.getByTestId(TestIds.passwordRequirements)).toBeVisible()
  await page.getByTestId(TestIds.changePasswordSubmitButton).click()

  await expect(page.getByTestId(TestIds.appShell)).toBeVisible({
    timeout: 60_000,
  })

  const allowed = await request.get(`${API_URL}/api/v1/auth/me`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  expect(allowed.ok()).toBeTruthy()
  const me = await allowed.json()
  expect(me.must_change_password).toBeFalsy()

  const status = await (await request.get(`${API_URL}/api/v1/auth/status`)).json()
  expect(status.password_policy?.min_length).toBeGreaterThanOrEqual(12)
})
