import { expect, test, type APIRequestContext } from '@playwright/test'
import { execFileSync } from 'node:child_process'

const ADMIN_EMAIL = process.env.E2E_EMAIL ?? 'e2e@vulyk.local'
const ADMIN_PASSWORD = process.env.E2E_PASSWORD ?? 'e2e-password-123'
const API_URL = process.env.VULYK_API_URL ?? 'http://127.0.0.1:8080'

async function ensureAdmin(request: APIRequestContext) {
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
    return
  }

  const login = await request.post(`${API_URL}/api/v1/auth/login`, {
    data: { email: ADMIN_EMAIL, password: ADMIN_PASSWORD },
  })
  if (login.ok()) return

  const databaseUrl = process.env.DATABASE_URL
  if (!databaseUrl) {
    throw new Error(
      `Cannot login as ${ADMIN_EMAIL} and DATABASE_URL unset for reset`,
    )
  }
  execFileSync(
    'psql',
    [databaseUrl, '-v', 'ON_ERROR_STOP=1', '-c', 'TRUNCATE users CASCADE'],
    { stdio: 'inherit' },
  )
  const res = await request.post(`${API_URL}/api/v1/auth/bootstrap`, {
    data: {
      email: ADMIN_EMAIL,
      password: ADMIN_PASSWORD,
      display_name: 'E2E Admin',
    },
  })
  expect(res.ok(), await res.text()).toBeTruthy()
}

async function loginIfNeeded(page: import('@playwright/test').Page) {
  await page.goto('/')
  await expect(page.locator('.login-panel, .app-shell').first()).toBeVisible({
    timeout: 60_000,
  })
  if (await page.locator('.login-panel').isVisible()) {
    const button = page.getByRole('button', { name: /Create admin|Sign in/i })
    if ((await button.textContent())?.includes('Create admin')) {
      await page.getByLabel('Display name').fill('E2E Admin')
    }
    await page.getByLabel('Email').fill(ADMIN_EMAIL)
    await page.getByLabel('Password').fill(ADMIN_PASSWORD)
    await button.click()
  }
  await expect(page.locator('.app-shell')).toBeVisible({ timeout: 60_000 })
}

test.describe('mobile viewport', () => {
  test.beforeAll(async ({ request }) => {
    await ensureAdmin(request)
  })

  test('bottom nav, suite drawer, and stacked case flow', async ({
    page,
    isMobile,
  }, testInfo) => {
    test.skip(
      !isMobile && !/Mobile/.test(testInfo.project.name),
      'mobile project only',
    )

    await loginIfNeeded(page)

    const bottom = page.getByRole('navigation', { name: 'Mobile primary' })
    await expect(bottom).toBeVisible()
    await expect(bottom.getByRole('link', { name: 'Cases' })).toBeVisible()
    await expect(bottom.getByRole('link', { name: 'Runs' })).toBeVisible()

    // Suite tree is a drawer on small screens
    await page.getByRole('button', { name: 'Suites' }).click()
    await expect(page.locator('#suite-drawer')).toBeVisible()
    await expect(page.getByRole('button', { name: 'All cases' })).toBeVisible()
    // Prefer the in-drawer Close control (backdrop sits under the panel on WebKit)
    await page.locator('#suite-drawer').getByRole('button', { name: 'Close' }).click()
    await expect(page.locator('.drawer-backdrop')).toHaveCount(0)

    // Case list create controls remain usable
    await expect(page.getByLabel('New case key')).toBeVisible()
    const caseKey = `MOB-${Date.now().toString().slice(-5)}`
    await page.getByLabel('New case key').fill(caseKey)
    await page.getByLabel('New case title').fill(`Mobile ${caseKey}`)
    await page.getByRole('button', { name: 'Create case' }).click()
    await expect(page.getByText(`Mobile ${caseKey}`)).toBeVisible({
      timeout: 30_000,
    })

    await page.getByText(`Mobile ${caseKey}`).click()
    await expect(page.getByText(caseKey).first()).toBeVisible()
    await expect(page.getByRole('link', { name: /Back to cases/i })).toBeVisible()

    await bottom.getByRole('link', { name: 'Runs' }).click()
    await expect(page.getByRole('heading', { name: 'Start a run' })).toBeVisible()

    await bottom.getByRole('link', { name: 'Admin' }).click()
    await expect(page.getByRole('heading', { name: 'Admin' })).toBeVisible()
    await expect(page.getByRole('tablist')).toBeVisible()
  })

  test('web app manifest and service worker are present', async ({ page }) => {
    await page.goto('/')
    const manifest = page.locator('link[rel="manifest"]')
    await expect(manifest).toHaveAttribute('href', /manifest\.webmanifest/)

    const manifestRes = await page.request.get('/manifest.webmanifest')
    expect(manifestRes.ok()).toBeTruthy()
    const body = await manifestRes.json()
    expect(body.name).toMatch(/Vulyk/i)
    expect(body.icons?.length).toBeGreaterThan(4)
    expect(body.icons.some((i: { purpose?: string }) => i.purpose?.includes('maskable'))).toBeTruthy()

    // After load, a controlling or waiting SW should register (production build)
    const swReady = await page.evaluate(async () => {
      if (!('serviceWorker' in navigator)) return false
      const reg = await navigator.serviceWorker.getRegistration()
      return Boolean(reg?.active || reg?.waiting || reg?.installing)
    })
    expect(swReady).toBeTruthy()
  })
})
