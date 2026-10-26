import { expect, type Page } from '@playwright/test'

/** Open the compact globe language menu in the topbar. */
export async function openLanguageMenu(page: Page) {
  const btn = page.getByRole('button', {
    name: /^(Language|Мова|Langue|Idioma|Sprache)$/i,
  })
  await expect(btn).toBeVisible()
  await btn.click()
  await expect(page.getByRole('menu')).toBeVisible()
}

/** Choose a locale by its native name in the globe menu. */
export async function chooseLocale(
  page: Page,
  nativeName: 'English' | 'Українська' | 'Français' | 'Español' | 'Deutsch',
) {
  await openLanguageMenu(page)
  await page.getByRole('menuitemradio', { name: nativeName }).click()
}

/** Open the gear user menu in the topbar. */
export async function openUserMenu(page: Page) {
  const btn = page.getByRole('button', {
    name: /User menu|Меню користувача|Menu utilisateur|Menú de usuario|Benutzermenü/i,
  })
  await expect(btn).toBeVisible()
  await btn.click()
  await expect(page.getByRole('menu')).toBeVisible()
}

export function userMenuItem(page: Page, name: RegExp) {
  return page.getByRole('menuitem', { name })
}
