import { expect, type Page } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'

/** Open the compact globe language menu in the topbar. */
export async function openLanguageMenu(page: Page) {
  const btn = page.getByTestId(TestIds.headerLanguageMenu)
  await expect(btn).toBeVisible()
  await btn.click()
  await expect(page.getByTestId(TestIds.headerLanguageMenuPanel)).toBeVisible()
}

/** Choose a locale by its code (en/uk/fr/es/de) via data-entity-key. */
export async function chooseLocale(
  page: Page,
  nativeName: 'English' | 'Українська' | 'Français' | 'Español' | 'Deutsch',
) {
  const codeByName: Record<string, string> = {
    English: 'en',
    Українська: 'uk',
    Français: 'fr',
    Español: 'es',
    Deutsch: 'de',
  }
  await openLanguageMenu(page)
  const code = codeByName[nativeName]
  await page
    .locator(
      `[data-testid="${TestIds.headerLanguageItem}"][data-entity-key="${code}"]`,
    )
    .click()
}

/** Open the gear user menu in the topbar. */
export async function openUserMenu(page: Page) {
  const btn = page.getByTestId(TestIds.headerGearMenu)
  await expect(btn).toBeVisible()
  await btn.click()
  await expect(page.getByTestId(TestIds.headerGearMenuPanel)).toBeVisible()
}

export function userMenuItem(page: Page, name: RegExp) {
  return page.getByRole('menuitem', { name })
}
