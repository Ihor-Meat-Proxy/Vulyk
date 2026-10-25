import AxeBuilder from '@axe-core/playwright'
import { expect, test, type Page } from '@playwright/test'
import { loginAsAdmin } from '../fixtures/auth'

const THEMES = [
  'system',
  'light',
  'dark',
  'high_contrast',
  'sepia',
] as const

async function openAppearance(page: Page) {
  await page.goto('/appearance')
  await expect(page.getByRole('heading', { name: /Appearance|Вигляд/i })).toBeVisible({
    timeout: 30_000,
  })
}

async function selectTheme(page: Page, theme: string) {
  await page.getByLabel(/Theme|Тема/i).selectOption(theme)
  await expect(page.locator('html')).toHaveAttribute(
    'data-theme-choice',
    theme,
    { timeout: 5_000 },
  )
}

test.describe('Appearance & theming', () => {
  test.beforeEach(async ({ page }) => {
    await loginAsAdmin(page)
  })

  test('switches each theme and persists after reload', async ({ page }) => {
    await openAppearance(page)

    for (const theme of THEMES) {
      await selectTheme(page, theme)
      const resolved = await page.locator('html').getAttribute('data-theme')
      expect(resolved).toBeTruthy()
      if (theme !== 'system') {
        expect(resolved).toBe(theme)
      }

      await page.reload()
      await expect(page.locator('html')).toHaveAttribute(
        'data-theme-choice',
        theme,
      )
      await openAppearance(page)
      await expect(page.getByLabel(/Theme|Тема/i)).toHaveValue(theme)
    }
  })

  test('persists theme after logout and login', async ({ page }) => {
    await openAppearance(page)
    await selectTheme(page, 'dark')

    const signOut = page.getByRole('button', { name: /Sign out|Вийти/i })
    await signOut.click()
    await expect(page.locator('.login-panel')).toBeVisible({ timeout: 30_000 })
    await expect(page.locator('html')).toHaveAttribute('data-theme-choice', 'dark')
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')

    await loginAsAdmin(page)
    await expect(page.locator('html')).toHaveAttribute('data-theme-choice', 'dark')
    await openAppearance(page)
    await expect(page.getByLabel(/Theme|Тема/i)).toHaveValue('dark')
  })

  test('status badges show icon and label', async ({ page }) => {
    await openAppearance(page)
    const preview = page.locator('.appearance-preview')
    await expect(preview).toBeVisible()

    for (const status of [
      'Passed',
      'Failed',
      'Blocked',
      'Skipped',
      'Retest',
      'Untested',
    ]) {
      const badge = preview.locator(`.status-pill[data-status="${status}"]`)
      await expect(badge).toBeVisible()
      await expect(badge.locator('.status-icon')).toBeVisible()
      await expect(badge.locator('.status-label')).toBeVisible()
      await expect(badge.locator('.status-label')).not.toBeEmpty()
    }
  })

  test('axe accessibility scan per theme', async ({ page }) => {
    await openAppearance(page)

    for (const theme of THEMES) {
      await selectTheme(page, theme)
      const results = await new AxeBuilder({ page })
        .withTags(['wcag2a', 'wcag2aa'])
        .disableRules(['color-contrast']) // token contrast covered by unit test
        .analyze()
      const serious = results.violations.filter(
        (v) => v.impact === 'critical' || v.impact === 'serious',
      )
      expect(
        serious,
        `axe serious/critical for theme=${theme}: ${JSON.stringify(
          serious.map((v) => ({ id: v.id, nodes: v.nodes.length })),
        )}`,
      ).toEqual([])
    }
  })

  test('font size and density apply to document', async ({ page }) => {
    await openAppearance(page)
    await page.getByRole('button', { name: '125%' }).click()
    const scale = await page.locator('html').evaluate((el) =>
      getComputedStyle(el).getPropertyValue('--font-scale').trim(),
    )
    expect(scale).toBe('1.25')

    await page.getByLabel(/Density|Щільність/i).selectOption('compact')
    await expect(page.locator('html')).toHaveAttribute('data-density', 'compact')

    await page.reload()
    await expect(page.locator('html')).toHaveAttribute('data-density', 'compact')
  })
})
