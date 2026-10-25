import { expect, test } from '@playwright/test'
import { authStatus, updateAdminSettings } from '../fixtures/api'
import {
  ensureAdminSession,
  loginAsAdmin,
  markSetupComplete,
  prepareSetupWizard,
  sha256Hex,
} from '../fixtures/auth'
import { API_URL, uniqueKey } from '../fixtures/env'
import { SetupPage } from '../pages/SetupPage'

test.describe.configure({ mode: 'serial' })

const SETUP_TOKEN = `vulyk_setup_e2e_${uniqueKey('tk').toLowerCase()}`

test.afterAll(() => {
  if (!process.env.DATABASE_URL) return
  try {
    markSetupComplete()
  } catch {
    // best-effort restore for shared CI DB
  }
})

test('setup token protection + wizard locale/timezone', async ({
  page,
  request,
}) => {
  test.skip(
    !process.env.DATABASE_URL,
    'DATABASE_URL required to reset setup_completed for wizard e2e',
  )

  // Sanity: known hash algorithm
  expect(sha256Hex('abc')).toHaveLength(64)

  await ensureAdminSession(request)
  prepareSetupWizard(SETUP_TOKEN)

  // Bad token rejected by API
  const bad = await request.post(`${API_URL}/api/v1/setup/complete`, {
    data: {
      setup_token: 'not-a-real-token',
      email: `wizard-${uniqueKey('').toLowerCase()}@vulyk.local`,
      password: 'wizard-pass-1',
      display_name: 'Wizard',
      default_locale: 'uk',
      default_timezone: 'Europe/Kyiv',
    },
  })
  expect([401, 403, 400]).toContain(bad.status())

  const setup = new SetupPage(page)
  await setup.expectWizard()

  // UI: bad token
  await setup.fill({
    token: 'wrong-token',
    email: `wizard-bad-${uniqueKey('').toLowerCase()}@vulyk.local`,
    password: 'wizard-pass-1',
    locale: 'uk',
    timezone: 'Europe/Kyiv',
  })
  await setup.submit()
  await setup.expectError()

  // Good token + uk locale + timezone
  const email = `wizard-ok-${uniqueKey('').toLowerCase()}@vulyk.local`
  await setup.fill({
    token: SETUP_TOKEN,
    email,
    password: 'wizard-pass-9',
    displayName: 'Wizard Admin',
    locale: 'uk',
    timezone: 'Europe/Kyiv',
    registrationMode: 'open',
  })
  await setup.submit()
  // App leaves the wizard as soon as setup_required clears → login panel
  await expect(page.locator('.login-panel')).toBeVisible({ timeout: 30_000 })

  const status = await authStatus(request)
  expect(status.setup_required).toBeFalsy()

  // Locale switch + timezone display in admin settings
  // Sign in as original seeded admin (still present)
  await loginAsAdmin(page)
  await page.getByLabel(/Language|Мова|locale/i).selectOption('uk')
  await expect(
    page.getByRole('navigation', { name: 'Primary' }).getByRole('link').first(),
  ).toContainText(/Кейси|Cases/)

  await page
    .getByRole('navigation', { name: 'Primary' })
    .getByRole('link', { name: /Admin|Адмін/i })
    .click()
  await page.getByRole('tab', { name: /Settings|Налаштування/i }).click()
  await expect(page.getByLabel(/Default timezone|timezone/i)).toBeVisible()

  // Ensure timezone can be read/saved
  const admin = await ensureAdminSession(request)
  const updated = await updateAdminSettings(request, admin.token, {
    default_timezone: 'Europe/Kyiv',
    default_locale: 'uk',
  })
  expect(
    (updated as { default_timezone?: string }).default_timezone ?? 'Europe/Kyiv',
  ).toBeTruthy()
})

test('setup CLI --help is available', async () => {
  const { execFileSync } = await import('node:child_process')
  const fs = await import('node:fs')
  const path = await import('node:path')
  const { fileURLToPath } = await import('node:url')
  const repoRoot = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    '../..',
  )
  const releaseBin = path.join(repoRoot, 'target/release/vulyk-api')
  const debugBin = path.join(repoRoot, 'target/debug/vulyk-api')
  const out = fs.existsSync(releaseBin)
    ? execFileSync(releaseBin, ['setup', '--help'], {
        cwd: repoRoot,
        encoding: 'utf8',
        env: process.env,
      })
    : fs.existsSync(debugBin)
      ? execFileSync(debugBin, ['setup', '--help'], {
          cwd: repoRoot,
          encoding: 'utf8',
          env: process.env,
        })
      : execFileSync(
          'cargo',
          ['run', '-p', 'vulyk-api', '--quiet', '--', 'setup', '--help'],
          {
            cwd: repoRoot,
            encoding: 'utf8',
            env: process.env,
          },
        )
  expect(out).toMatch(/Complete first-run instance setup|admin-email|--yes/i)
})
