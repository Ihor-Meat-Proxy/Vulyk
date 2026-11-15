import { expect, test } from '@playwright/test'
import { loginAsAdmin } from '../fixtures/auth'

test('theme-init.js applies stored theme without CSP violations', {
  tag: ['@VQA-338', '@VUE-112'],
  annotation: [
    { type: 'vulyk', description: 'VQA-338' },
    { type: 'vulyk', description: 'VUE-112' },
  ],
}, async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem(
      'vulyk.appearance',
      JSON.stringify({
        theme: 'dark',
        fontSize: 100,
        fontFamily: 'inter',
        density: 'comfortable',
        reducedMotion: null,
      }),
    )
  })

  const consoleErrors: string[] = []
  page.on('console', (msg) => {
    if (msg.type() === 'error') consoleErrors.push(msg.text())
  })
  page.on('pageerror', (err) => consoleErrors.push(String(err)))

  // theme-init.js must run before paint; verify attributes before React hydrates heavily.
  await page.goto('/', { waitUntil: 'domcontentloaded' })
  await expect(page.locator('html')).toHaveAttribute('data-theme-choice', 'dark')
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')

  const themeInit = page.locator('script[src="/theme-init.js"]')
  await expect(themeInit).toHaveCount(1)
  // No inline theme bootstrap scripts in the document head.
  const inlineTheme = await page.evaluate(() => {
    const scripts = Array.from(document.querySelectorAll('head script:not([src])'))
    return scripts.some((s) => /vulyk\.appearance|data-theme/.test(s.textContent ?? ''))
  })
  expect(inlineTheme).toBe(false)

  const cspBlocked = consoleErrors.filter((e) =>
    /Content Security Policy|Refused to execute inline script|inline script/i.test(e),
  )
  expect(cspBlocked, `CSP errors: ${cspBlocked.join('\n')}`).toEqual([])

  await loginAsAdmin(page)
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')
})
