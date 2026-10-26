import { expect, test } from '@playwright/test'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { chooseLocale, openLanguageMenu } from '../fixtures/header'

test.describe.configure({ mode: 'serial' })

test.beforeAll(async ({ request }) => {
  await ensureAdminSession(request)
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
