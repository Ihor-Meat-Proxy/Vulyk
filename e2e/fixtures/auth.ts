import { expect, type APIRequestContext, type Page } from '@playwright/test'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { TestIds } from '../../frontend/src/testing/testIds'
import {
  ADMIN_EMAIL,
  ADMIN_PASSWORD,
  API_URL,
  DEMO_EMAIL,
  DEMO_PASSWORD,
  uniqueKey,
} from './env'
import {
  createUser,
  loginApi,
  upsertMembership,
  type AuthSession,
} from './api'

export { ADMIN_EMAIL, ADMIN_PASSWORD, DEMO_EMAIL, DEMO_PASSWORD, API_URL }

export function sha256Hex(value: string): string {
  return createHash('sha256').update(value).digest('hex')
}

/** Ensure seeded admin can authenticate (or bootstrap if empty DB). */
export async function ensureAdminSession(
  request: APIRequestContext,
): Promise<AuthSession> {
  const login = await request.post(`${API_URL}/api/v1/auth/login`, {
    data: { email: ADMIN_EMAIL, password: ADMIN_PASSWORD },
  })
  if (login.ok()) {
    const body = await login.json()
    return { token: body.token, user: body.user }
  }

  const status = await (await request.get(`${API_URL}/api/v1/auth/status`)).json()
  if (status.bootstrap_available) {
    const res = await request.post(`${API_URL}/api/v1/auth/bootstrap`, {
      data: {
        email: ADMIN_EMAIL,
        password: ADMIN_PASSWORD,
        display_name: 'E2E Admin',
      },
    })
    expect(res.ok(), await res.text()).toBeTruthy()
    const body = await res.json()
    return { token: body.token, user: body.user }
  }

  throw new Error(
    `Cannot login as ${ADMIN_EMAIL} (status ${login.status()}). ` +
      'Start API with VULYK_ADMIN_PASSWORD / VULYK_SEED_DEMO seeding.',
  )
}

export async function loginAs(
  page: Page,
  email: string,
  password: string,
): Promise<void> {
  await page.goto('/')
  await expect(page.locator('.login-panel, .app-shell').first()).toBeVisible({
    timeout: 60_000,
  })

  if (await page.locator('.app-shell').isVisible()) {
    // Already authenticated — sign out via gear menu (or clear session).
    const gear = page.getByRole('button', {
      name: /User menu|Меню користувача|Menu utilisateur|Menú de usuario|Benutzermenü/i,
    })
    if (await gear.isVisible().catch(() => false)) {
      await gear.click()
      const signOut = page.getByRole('menuitem', {
        name: /Sign out|Вийти|Se déconnecter|Cerrar sesión|Abmelden/i,
      })
      if (await signOut.isVisible().catch(() => false)) {
        await signOut.click()
      }
    }
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })
    await page.goto('/login')
  }

  const panel = page.locator('.login-panel')
  await expect(panel).toBeVisible({ timeout: 30_000 })
  const submit = panel.locator('button[type="submit"]')
  if ((await submit.textContent())?.match(/Create admin|Створити/i)) {
    await page.getByLabel(/Display name|Ім'я|Імʼя/i).fill('E2E Admin')
  }
  await page.getByLabel(/^Email$/i).fill(email)
  await page.getByLabel(/Password|Пароль/i).fill(password)
  await submit.click()
  await expect(page.locator('.app-shell')).toBeVisible({ timeout: 60_000 })
}

/** Dismiss PWA offline toasts that intercept pointer events over the main UI. */
export async function dismissPwaToasts(page: Page): Promise<void> {
  const dismiss = page.getByRole('button', { name: 'Dismiss' })
  for (let i = 0; i < 3; i++) {
    if (!(await dismiss.first().isVisible().catch(() => false))) break
    await dismiss.first().click({ timeout: 2_000 }).catch(() => undefined)
  }
}

/**
 * Force English UI for the current session. Locale-switch specs persist
 * `users.locale` / localStorage; most other specs still use en|uk-only locators.
 */
export async function pinEnglishLocale(page: Page): Promise<void> {
  await page.evaluate(() => {
    localStorage.setItem('vulyk.locale', 'en')
  })
  const lang = await page.locator('html').getAttribute('lang')
  if (lang === 'en') return

  await page.evaluate(async () => {
    const token = localStorage.getItem('vulyk.authToken')
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    }
    if (token) headers.Authorization = `Bearer ${token}`
    await fetch('/api/v1/auth/me', {
      method: 'PATCH',
      headers,
      credentials: 'include',
      body: JSON.stringify({ locale: 'en' }),
    })
  })
  await page.reload()
  await expect(page.locator('.app-shell, .login-panel').first()).toBeVisible({
    timeout: 60_000,
  })
  await expect(page.locator('html')).toHaveAttribute('lang', 'en')
  await dismissPwaToasts(page)
}

export async function loginAsAdmin(page: Page): Promise<void> {
  await loginAs(page, ADMIN_EMAIL, ADMIN_PASSWORD)
  await dismissPwaToasts(page)
  await pinEnglishLocale(page)
}

export async function loginAsDemo(page: Page): Promise<void> {
  await loginAs(page, DEMO_EMAIL, DEMO_PASSWORD)
}

export async function logout(page: Page): Promise<void> {
  const gear = page.getByTestId(TestIds.headerGearMenu)
  await expect(gear).toBeVisible()
  await gear.click()
  const signOut = page.getByTestId(TestIds.headerGearSignOut)
  await expect(signOut).toBeVisible()
  // Wait for the logout request to finish before any navigation. A premature
  // page.goto('/login') aborts the fetch, leaves the session cookie, and the
  // app redirects authenticated users away from /login back to the shell.
  await Promise.all([
    page.waitForResponse(
      (r) =>
        r.url().includes('/api/v1/auth/logout') &&
        r.request().method() === 'POST',
      { timeout: 15_000 },
    ),
    signOut.click(),
  ])
  await page.evaluate(() => {
    localStorage.removeItem('vulyk.authToken')
  })
  const panel = page.getByTestId(TestIds.loginPanel)
  try {
    await expect(panel).toBeVisible({ timeout: 10_000 })
  } catch {
    await page.goto('/login')
    await expect(panel).toBeVisible({ timeout: 30_000 })
  }
}

/** Create a user and assign a project role via API (admin token). */
export async function createUserWithRole(
  request: APIRequestContext,
  adminToken: string,
  projectId: string,
  role: string,
  opts: { email?: string; password?: string; displayName?: string } = {},
): Promise<{ email: string; password: string; userId: string; session: AuthSession }> {
  const email =
    opts.email ?? `e2e-${role.replace(/_/g, '-')}-${uniqueKey('').toLowerCase()}@vulyk.local`
  const password = opts.password ?? 'e2e-pass-12345'
  const user = await createUser(request, adminToken, {
    email,
    password,
    display_name: opts.displayName ?? `E2E ${role}`,
  })
  await upsertMembership(request, adminToken, projectId, user.id, role)
  const session = await loginApi(request, email, password)
  return { email, password, userId: user.id, session }
}

/** Store JWT in localStorage so UI boots authenticated without form. */
export async function injectApiToken(page: Page, token: string): Promise<void> {
  await page.addInitScript((t) => {
    localStorage.setItem('vulyk.authToken', t)
  }, token)
  // Also set for already-loaded pages
  await page.goto('/')
  await page.evaluate((t) => {
    localStorage.setItem('vulyk.authToken', t)
  }, token)
}

export function runSql(sql: string): void {
  const databaseUrl = process.env.DATABASE_URL
  if (!databaseUrl) {
    throw new Error('DATABASE_URL is required for SQL helpers')
  }
  execFileSync(
    'psql',
    [databaseUrl, '-v', 'ON_ERROR_STOP=1', '-c', sql],
    { stdio: 'pipe' },
  )
}

/** Reset setup so the wizard is shown; inserts a known one-time token. */
export function prepareSetupWizard(rawToken: string): void {
  const hash = sha256Hex(rawToken)
  runSql(`
    UPDATE system_settings SET value = 'false'::jsonb WHERE key = 'setup_completed';
    UPDATE system_settings SET value = '"en"'::jsonb WHERE key = 'default_locale';
    DELETE FROM setup_tokens;
    INSERT INTO setup_tokens (token_hash) VALUES ('${hash}');
  `)
}

export function markSetupComplete(): void {
  runSql(`
    UPDATE system_settings SET value = 'true'::jsonb WHERE key = 'setup_completed';
    UPDATE setup_tokens SET consumed_at = NOW() WHERE consumed_at IS NULL;
  `)
}
