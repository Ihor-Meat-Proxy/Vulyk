import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import {
  API_URL,
  ensureAdminSession,
  loginAsAdmin,
  logout,
} from '../fixtures/auth'
import { chooseLocale, openLanguageMenu } from '../fixtures/header'

test.describe.configure({ mode: 'serial' })

test.beforeAll(async ({ request }) => {
  await ensureAdminSession(request)
})

test.afterAll(async ({ request }) => {
  // Undo users.locale mutations so later specs see English chrome.
  const admin = await ensureAdminSession(request)
  await request.patch(`${API_URL}/api/v1/auth/me`, {
    headers: { Authorization: `Bearer ${admin.token}` },
    data: { locale: 'en' },
  })
})

test('globe menu lists native names and switches each locale', async ({
  page,
}) => {
  await loginAsAdmin(page)

  await openLanguageMenu(page)
  for (const name of [
    'English',
    'Українська',
    'Français',
    'Español',
    'Deutsch',
  ]) {
    await expect(page.getByRole('menuitemradio', { name })).toBeVisible()
  }
  await page.keyboard.press('Escape')

  const cases: Array<{
    native: 'English' | 'Українська' | 'Français' | 'Español' | 'Deutsch'
    lang: string
    nav: RegExp
  }> = [
    { native: 'Українська', lang: 'uk', nav: /Кейси/ },
    { native: 'Français', lang: 'fr', nav: /Cas de test/ },
    { native: 'Español', lang: 'es', nav: /Casos de prueba/ },
    { native: 'Deutsch', lang: 'de', nav: /Testfälle/ },
    { native: 'English', lang: 'en', nav: /Cases/ },
  ]

  const primary = page.getByRole('navigation', { name: 'Primary' })
  for (const { native, lang, nav } of cases) {
    await chooseLocale(page, native)
    await expect(page.locator('html')).toHaveAttribute('lang', lang)
    await expect(primary.getByRole('link').first()).toContainText(nav)
    await openLanguageMenu(page)
    await expect(
      page.getByRole('menuitemradio', { name: native }),
    ).toHaveAttribute('aria-checked', 'true')
    await page.keyboard.press('Escape')
  }
})

test('locale persists for signed-in user and in localStorage', async ({
  page,
  request,
}) => {
  const admin = await ensureAdminSession(request)
  await loginAsAdmin(page)
  await chooseLocale(page, 'Français')
  await expect(page.locator('html')).toHaveAttribute('lang', 'fr')

  const stored = await page.evaluate(() => localStorage.getItem('vulyk.locale'))
  expect(stored).toBe('fr')

  await expect
    .poll(async () => {
      const res = await request.get(`${API_URL}/api/v1/auth/me`, {
        headers: { Authorization: `Bearer ${admin.token}` },
      })
      if (!res.ok()) return null
      const body = (await res.json()) as { locale?: string | null }
      return body.locale ?? null
    })
    .toBe('fr')

  await page.reload()
  await expect(page.locator('html')).toHaveAttribute('lang', 'fr')
  await expect(
    page.getByRole('navigation', { name: 'Primary' }).getByRole('link').first(),
  ).toContainText(/Cas de test/)

  // Restore English for later specs.
  await chooseLocale(page, 'English')
})

test('anonymous login page keeps locale from localStorage', async ({ page }) => {
  await loginAsAdmin(page)
  await chooseLocale(page, 'Deutsch')
  await expect(page.locator('html')).toHaveAttribute('lang', 'de')

  await logout(page)
  await expect(page.getByTestId(TestIds.loginPanel)).toBeVisible()
  await expect(page.locator('html')).toHaveAttribute('lang', 'de')
  const stored = await page.evaluate(() => localStorage.getItem('vulyk.locale'))
  expect(stored).toBe('de')
  await expect(page.getByRole('heading').or(page.locator('.brand-tag'))).toContainText(
    /Anmelden|Sign in|Connexion|Iniciar|Увійти/i,
  )
  // German sign-in label
  await expect(page.locator('.brand-tag')).toContainText(/Anmelden/)

  await openLanguageMenu(page)
  await page
    .locator(
      `[data-testid="${TestIds.headerLanguageItem}"][data-entity-key="en"]`,
    )
    .click()
  await expect(page.locator('html')).toHaveAttribute('lang', 'en')
})

test('locale-aware number formatting on Cases', async ({ page }) => {
  await loginAsAdmin(page)
  await chooseLocale(page, 'English')
  await page
    .getByRole('navigation', { name: 'Primary' })
    .getByRole('link', { name: /Cases|Кейси|Cas de test|Casos|Testfälle/i })
    .click()
  await expect(page.locator('html')).toHaveAttribute('lang', 'en')

  const formats = await page.evaluate(() => {
    const n = 1234
    return {
      en: new Intl.NumberFormat('en-US').format(n),
      de: new Intl.NumberFormat('de-DE').format(n),
      fr: new Intl.NumberFormat('fr-FR').format(n),
    }
  })
  expect(formats.en).not.toBe(formats.de)
  expect(formats.de).toContain('.')

  await chooseLocale(page, 'Deutsch')
  await expect(page.locator('html')).toHaveAttribute('lang', 'de')
  await expect(page.locator('.muted').filter({ hasText: /Fälle/i }).first()).toBeVisible({
    timeout: 15_000,
  })

  await chooseLocale(page, 'English')
})

test('API errors still display under non-English locale', async ({ page }) => {
  await loginAsAdmin(page)
  await chooseLocale(page, 'Français')
  await logout(page)

  await page.getByTestId(TestIds.loginEmailInput).fill('nobody@example.com')
  await page.getByTestId(TestIds.loginPasswordInput).fill('wrong-password-99')
  await page.getByTestId(TestIds.loginSubmitButton).click()
  await expect(page.getByTestId(TestIds.loginError)).toBeVisible({ timeout: 15_000 })
  await expect(page.getByTestId(TestIds.loginError)).not.toHaveText('')
  // Login chrome remains French (localStorage) while error text may be English from API.
  await expect(page.locator('html')).toHaveAttribute('lang', 'fr')
  await expect(page.locator('.brand-tag')).toContainText(/Connexion|Sign in/i)

  // Restore English for subsequent specs (loginAsAdmin pins, but clear localStorage too).
  await chooseLocale(page, 'English')
  await expect(page.locator('html')).toHaveAttribute('lang', 'en')
})
