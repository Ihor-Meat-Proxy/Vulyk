import { expect, test, type APIRequestContext } from '@playwright/test'
import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const fixtures = path.join(__dirname, '..', 'fixtures')

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

  // Local re-runs against a dirty DB: reset users when DATABASE_URL is set.
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

test('login, create case, run, upload JUnit, import TestLink, view report', async ({
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
  await expect(page.getByRole('navigation')).toContainText('Cases')

  await expect(page.locator('.project-select')).toBeVisible()
  await expect
    .poll(async () => page.locator('.project-select option').count())
    .toBeGreaterThan(0)

  const caseKey = `E2E-${Date.now().toString().slice(-6)}`
  const caseTitle = `E2E case ${caseKey}`
  await page.getByLabel('New case key').fill(caseKey)
  await page.getByLabel('New case title').fill(caseTitle)
  await page.getByRole('button', { name: 'Create case' }).click()
  await expect(page.getByText(caseTitle)).toBeVisible({ timeout: 30_000 })

  const nav = page.getByRole('navigation', { name: 'Primary' })
  await nav.getByRole('link', { name: 'Runs', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Start a run' })).toBeVisible()
  const runName = `E2E run ${caseKey}`
  await page.getByPlaceholder('Run name').fill(runName)
  await page.getByRole('button', { name: 'Create run' }).click()
  await expect(page.getByText(runName).first()).toBeVisible({ timeout: 30_000 })

  await nav.getByRole('link', { name: 'Runs', exact: true }).click()
  await expect(
    page.getByRole('heading', { name: 'Upload JUnit results' }),
  ).toBeVisible()
  await page.getByLabel('Upload target run').selectOption({ label: runName })
  await page
    .getByLabel('JUnit XML file')
    .setInputFiles(path.join(fixtures, 'sample-junit.xml'))
  await expect(page.getByText(/Ingested \d+ results/i)).toBeVisible({
    timeout: 30_000,
  })

  await nav.getByRole('link', { name: 'Import', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Import cases' })).toBeVisible()
  await page.getByLabel('Importer').selectOption({ label: 'TestLink' })
  await expect(page.getByLabel('Format')).toHaveValue('xml')
  const xml = fs.readFileSync(path.join(fixtures, 'testlink-sample.xml'), 'utf8')
  await page.getByLabel('Source content').fill(xml)
  await page.getByRole('button', { name: 'Import', exact: true }).click()
  await expect(page.getByText(/Created \d+ cases/i)).toBeVisible({
    timeout: 30_000,
  })

  await nav.getByRole('link', { name: 'Reports', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Reports' })).toBeVisible()
  await expect(page.getByText('Recent run summaries')).toBeVisible()
  await expect(page.locator('.kpi-value').first()).toBeVisible()
  await expect(page.locator('.report-list li').first()).toBeVisible({
    timeout: 30_000,
  })
})
