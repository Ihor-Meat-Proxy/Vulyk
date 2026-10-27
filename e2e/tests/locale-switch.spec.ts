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
  const admin = await ensureAdminSession(request)
  await request.patch(`${API_URL}/api/v1/auth/me`, {
    headers: { Authorization: `Bearer ${admin.token}` },
    data: { locale: 'en' },
  })
})

const LOCALES: Array<{
  native: 'English' | 'Українська' | 'Français' | 'Español' | 'Deutsch'
  code: string
}> = [
  { native: 'Українська', code: 'uk' },
  { native: 'Français', code: 'fr' },
  { native: 'Español', code: 'es' },
  { native: 'Deutsch', code: 'de' },
  { native: 'English', code: 'en' },
]

test('globe menu lists locales and switches via entity keys', async ({
  page,
}) => {
  await loginAsAdmin(page)

  await openLanguageMenu(page)
  for (const { code } of LOCALES) {
    await expect(
      page.locator(
        `[data-testid="${TestIds.headerLanguageItem}"][data-entity-key="${code}"]`,
      ),
    ).toBeVisible()
  }
  await page.keyboard.press('Escape')

  for (const { native, code } of LOCALES) {
    await chooseLocale(page, native)
    await expect(page.locator('html')).toHaveAttribute('lang', code)
    await expect(page.getByTestId(TestIds.navLinkCases)).toBeVisible()
    await openLanguageMenu(page)
    await expect(
      page.locator(
        `[data-testid="${TestIds.headerLanguageItem}"][data-entity-key="${code}"]`,
      ),
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
  await expect(page.getByTestId(TestIds.navLinkCases)).toBeVisible()

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
  await expect(page.getByTestId(TestIds.loginBrandTag)).toBeVisible()

  await openLanguageMenu(page)
  await page
    .locator(
      `[data-testid="${TestIds.headerLanguageItem}"][data-entity-key="en"]`,
    )
    .click()
  await expect(page.locator('html')).toHaveAttribute('lang', 'en')
})

test('Cases nav remains reachable under each locale', async ({ page }) => {
  await loginAsAdmin(page)
  await chooseLocale(page, 'English')
  await page.getByTestId(TestIds.navLinkCases).click()
  await expect(page.locator('html')).toHaveAttribute('lang', 'en')

  await chooseLocale(page, 'Deutsch')
  await expect(page.locator('html')).toHaveAttribute('lang', 'de')
  await expect(page.getByTestId(TestIds.navLinkCases)).toBeVisible()

  await chooseLocale(page, 'English')
})

test('API errors still display under non-English locale', async ({ page }) => {
  await loginAsAdmin(page)
  await chooseLocale(page, 'Français')
  await logout(page)

  await page.getByTestId(TestIds.loginEmailInput).fill('nobody@example.com')
  await page.getByTestId(TestIds.loginPasswordInput).fill('wrong-password-99')
  await page.getByTestId(TestIds.loginSubmitButton).click()
  await expect(page.getByTestId(TestIds.loginError)).toBeVisible({
    timeout: 15_000,
  })
  await expect(page.getByTestId(TestIds.loginError)).not.toHaveText('')
  await expect(page.locator('html')).toHaveAttribute('lang', 'fr')
  await expect(page.getByTestId(TestIds.loginBrandTag)).toBeVisible()

  await chooseLocale(page, 'English')
  await expect(page.locator('html')).toHaveAttribute('lang', 'en')
})
