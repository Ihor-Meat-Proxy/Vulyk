import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'

test('revoked bearer token redirects to login with session-expired notice', {
  tag: ['@VUE-234'],
  annotation: [{ type: 'vulyk', description: 'VUE-234' }],
}, async ({ page, request }) => {
  await ensureAdminSession(request)
  await loginAsAdmin(page)
  await expect(page.getByTestId(TestIds.appShell)).toBeVisible({
    timeout: 60_000,
  })

  // Poison the stored JWT/API token AND drop the session cookie. A bad Bearer
  // alone still falls through to a valid cookie (by design); both must be
  // invalid to force 401 → login (upgrade / revoke / expiry).
  await page.evaluate(() => {
    localStorage.setItem('vulyk.authToken', 'vulyk_at_revoked_for_e2e_test')
  })
  await page.context().clearCookies()

  await page.reload()
  await expect(page.getByTestId(TestIds.loginPanel)).toBeVisible({
    timeout: 60_000,
  })
  await expect(
    page.getByTestId(TestIds.loginSessionExpiredNotice),
  ).toBeVisible({ timeout: 15_000 })
  await expect(page.getByTestId(TestIds.appShell)).toHaveCount(0)
})
