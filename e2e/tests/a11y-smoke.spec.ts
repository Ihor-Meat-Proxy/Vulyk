/**
 * Fast axe smoke on the five primary surfaces (login, cases, runs, docs,
 * settings). Fails on serious/critical impacts. Used by the Fast CI gate
 * `Accessibility smoke` via Playwright project `a11y-smoke`.
 */
import AxeBuilder from '@axe-core/playwright'
import { expect, test, type Page } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { SettingsPage } from '../pages/SettingsPage'

test.describe.configure({ mode: 'serial' })

async function expectNoSeriousAxe(page: Page, label: string) {
  const results = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa'])
    // Match a11y-pass / appearance: contrast depends on webfont load timing.
    .disableRules(['color-contrast'])
    .analyze()
  const serious = results.violations.filter(
    (v) => v.impact === 'critical' || v.impact === 'serious',
  )
  expect(
    serious,
    `axe serious/critical on ${label}: ${JSON.stringify(
      serious.map((v) => ({
        id: v.id,
        impact: v.impact,
        nodes: v.nodes.length,
        help: v.help,
      })),
    )}`,
  ).toEqual([])
}

test('axe smoke: login + cases + runs + docs + settings', async ({
  page,
  request,
}) => {
  // --- Login (signed out) ---
  await page.goto('/')
  await expect(page.getByTestId(TestIds.loginPanel)).toBeVisible({
    timeout: 60_000,
  })
  await expectNoSeriousAxe(page, 'login')

  // --- Authenticated shell ---
  await ensureAdminSession(request)
  await loginAsAdmin(page)

  // Cases (index)
  await page.goto('/')
  await expect(page.getByTestId(TestIds.appShell)).toBeVisible({ timeout: 60_000 })
  await expect(page.getByTestId(TestIds.caseListPanel)).toBeVisible({
    timeout: 30_000,
  })
  await expectNoSeriousAxe(page, 'cases')

  // Runs
  await page.goto('/runs')
  await expect(page.getByTestId(TestIds.runsPage)).toBeVisible({
    timeout: 30_000,
  })
  await expectNoSeriousAxe(page, 'runs')

  // Docs
  await page.goto('/docs')
  await expect(page.getByTestId(TestIds.docsPage)).toBeVisible({
    timeout: 30_000,
  })
  await expectNoSeriousAxe(page, 'docs')

  // Settings
  const settings = new SettingsPage(page)
  await settings.open()
  await expectNoSeriousAxe(page, 'settings')
})
