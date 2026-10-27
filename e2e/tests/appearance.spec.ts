import AxeBuilder from '@axe-core/playwright'
import { expect, test, type Page } from '@playwright/test'
import { loginAsAdmin, logout } from '../fixtures/auth'

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

function themeSelect(page: Page) {
  return page.locator('.appearance-page select').nth(0)
}

function densitySelect(page: Page) {
  return page.locator('.appearance-page select').nth(1)
}

async function selectTheme(page: Page, theme: string) {
  await themeSelect(page).selectOption(theme)
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

  test('switches each theme and persists after reload', {
  tag: '@VQA-231', annotation: { type: 'vulyk', description: 'VQA-231' },
}, async ({ page }) => {
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
      await expect(themeSelect(page)).toHaveValue(theme)
    }
  })

  test('persists theme after logout and login', {
  tag: '@VQA-232', annotation: { type: 'vulyk', description: 'VQA-232' },
}, async ({ page }) => {
    await openAppearance(page)
    await selectTheme(page, 'dark')

    await logout(page)
    await expect(page.locator('html')).toHaveAttribute('data-theme-choice', 'dark')
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')

    await loginAsAdmin(page)
    await expect(page.locator('html')).toHaveAttribute('data-theme-choice', 'dark')
    await openAppearance(page)
    await expect(themeSelect(page)).toHaveValue('dark')
  })

  test('status badges show icon and label', {
  tag: '@VQA-233', annotation: { type: 'vulyk', description: 'VQA-233' },
}, async ({ page }) => {
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

  test('axe accessibility scan per theme', {
  tag: '@VQA-234', annotation: { type: 'vulyk', description: 'VQA-234' },
}, async ({ page }) => {
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

  test('font size and density apply to document', {
  tag: '@VQA-235', annotation: { type: 'vulyk', description: 'VQA-235' },
}, async ({ page }) => {
    await openAppearance(page)
    await page.getByRole('button', { name: '125%' }).click()
    const scale = await page.locator('html').evaluate((el) =>
      getComputedStyle(el).getPropertyValue('--font-scale').trim(),
    )
    expect(scale).toBe('1.25')

    await densitySelect(page).selectOption('compact')
    await expect(page.locator('html')).toHaveAttribute('data-density', 'compact')

    await page.reload()
    await expect(page.locator('html')).toHaveAttribute('data-density', 'compact')
  })
})
