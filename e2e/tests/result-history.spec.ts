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

test.describe.configure({ mode: 'serial' })

test.beforeAll(async ({ request }) => {
  await ensureAdmin(request)
})

test('mark Passed→Failed→Blocked→Skipped; timeline and case History tab', async ({
  page,
}) => {
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

  const caseKey = `HIST-${Date.now().toString().slice(-6)}`
  const caseTitle = `History case ${caseKey}`
  await page.getByLabel('New case key').fill(caseKey)
  await page.getByLabel('New case title').fill(caseTitle)
  await page.getByRole('button', { name: 'Create case' }).click()
  await expect(page.getByText(caseTitle)).toBeVisible({ timeout: 30_000 })

  await page.getByText(caseTitle).click()
  await expect(page.getByRole('heading', { level: 2 })).toContainText(caseTitle)
  await page.getByRole('button', { name: 'Edit' }).click()
  await page.getByRole('button', { name: 'Add step' }).click()
  await page.locator('.step textarea').first().fill('Open app')
  await page.locator('.step textarea').nth(1).fill('App loads')
  await page.getByRole('button', { name: 'Save' }).click()
  await expect(page.getByText('Open app')).toBeVisible()

  const nav = page.getByRole('navigation', { name: 'Primary' })
  await nav.getByRole('link', { name: 'Runs', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Start a run' })).toBeVisible()
  const runName = `History run ${caseKey}`
  await page.getByPlaceholder('Run name').fill(runName)
  await page.getByRole('button', { name: 'Create run' }).click()
  await expect(page.getByText(runName).first()).toBeVisible({ timeout: 30_000 })

  // Open the run detail (prefer link/click on run name in list).
  await page.getByText(runName).first().click()
  await expect(page.getByRole('heading', { name: runName })).toBeVisible({
    timeout: 30_000,
  })

  await page.getByRole('button', { name: new RegExp(caseKey) }).first().click()
  await expect(page.getByRole('group', { name: 'Result status' })).toBeVisible()

  for (const status of ['Passed', 'Failed', 'Blocked', 'Skipped'] as const) {
    await page.getByRole('button', { name: status, exact: true }).click()
    // After save, UI may advance; re-open the same case if needed.
    const stillOpen = await page
      .getByRole('group', { name: 'Result status' })
      .isVisible()
      .catch(() => false)
    if (!stillOpen) {
      await page.getByRole('button', { name: new RegExp(caseKey) }).first().click()
      await expect(page.getByRole('group', { name: 'Result status' })).toBeVisible()
    }
  }

  // Ensure case is selected and timeline shows all four in order.
  await page.getByRole('button', { name: new RegExp(caseKey) }).first().click()
  const timeline = page.getByLabel('Result history')
  await expect(timeline).toBeVisible()
  const pills = timeline.locator('.status-pill')
  await expect(pills).toHaveCount(4)
  await expect(pills.nth(0)).toHaveText('Passed')
  await expect(pills.nth(1)).toHaveText('Failed')
  await expect(pills.nth(2)).toHaveText('Blocked')
  await expect(pills.nth(3)).toHaveText('Skipped')

  // Mobile Skip is a primary thumb button (same group).
  await expect(
    page.getByRole('button', { name: 'Skipped', exact: true }),
  ).toBeVisible()

  // Case History / Results tab.
  await nav.getByRole('link', { name: 'Cases', exact: true }).click()
  await page.getByText(caseTitle).click()
  await page.getByRole('tab', { name: 'History / Results' }).click()
  const caseTimeline = page.getByLabel('Case results across runs')
  await expect(caseTimeline).toBeVisible({ timeout: 30_000 })
  const casePills = caseTimeline.locator('.status-pill')
  // Newest first on case page.
  await expect(casePills.first()).toHaveText('Skipped')
  await expect(casePills).toHaveCount(4)
  await expect(page.getByText(/Pass rate:/)).toBeVisible()
  await expect(page.getByText(/Latest:/)).toBeVisible()
})
