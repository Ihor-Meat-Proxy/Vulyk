import { expect, test } from '@playwright/test'

// Prefer tag + annotation so Vulyk (and check scripts) can map the case key.
// Fixture HTML avoids depending on external network from CI sandboxes.
const FIXTURE = 'data:text/html,<html><body><h1>Vulyk demo</h1></body></html>'

test(
  'KOC-12 Demo heading is visible',
  {
    tag: '@KOC-12',
    annotation: { type: 'vulyk', description: 'KOC-12' },
  },
  async ({ page }) => {
    await page.goto(FIXTURE)
    await expect(page.locator('h1')).toHaveText('Vulyk demo')
  },
)
