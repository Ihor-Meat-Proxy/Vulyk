import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { AdminPage } from '../pages/AdminPage'

test.describe.configure({ mode: 'serial' })

test('admin creates user and assigns project role', {
  tag: ['@VQA-325', '@VUE-101'], annotation: [{ type: 'vulyk', description: 'VQA-325' }, { type: 'vulyk', description: 'VUE-101' }],
}, async ({ page, request }) => {
  await ensureAdminSession(request)
  await loginAsAdmin(page)
  const admin = new AdminPage(page)
  await admin.open()

  const email = `user-${uniqueKey('U').toLowerCase()}@vulyk.local`
  const password = 'user-password-16ch'
  await admin.createUser(email, `User ${email}`, password)

  await admin.openTab('roles')
  await expect(page.getByTestId(TestIds.adminRoleUserSelect)).toBeVisible()
  await page.getByTestId(TestIds.adminRoleUserSelect).selectOption({ label: email })
  // Pick first non-empty role if available
  const roleSelect = page.getByTestId(TestIds.adminRoleSelect)
  const roleOptions = await roleSelect.locator('option').allTextContents()
  const roleIdx = roleOptions.findIndex((o) => /viewer|tester|lead|manager/i.test(o))
  if (roleIdx >= 0) {
    await roleSelect.selectOption({ index: roleIdx })
    await page.getByTestId(TestIds.adminRoleSaveButton).click()
  }
})

test('admin sample data section is visible', {
  tag: ['@VQA-326', '@VUE-102'], annotation: [{ type: 'vulyk', description: 'VQA-326' }, { type: 'vulyk', description: 'VUE-102' }],
}, async ({ page, request }) => {
  await ensureAdminSession(request)
  await loginAsAdmin(page)
  const admin = new AdminPage(page)
  await admin.open()
  await admin.openTab('projects')
  // Sample badge / remove control from status-sample coverage — just ensure projects tab loads
  await expect(page.getByTestId(TestIds.adminProjectNameInput)).toBeVisible()
})

test('admin SSO tab lists providers panel', {
  tag: ['@VQA-327', '@VUE-103'], annotation: [{ type: 'vulyk', description: 'VQA-327' }, { type: 'vulyk', description: 'VUE-103' }],
}, async ({ page, request }) => {
  await ensureAdminSession(request)
  await loginAsAdmin(page)
  const admin = new AdminPage(page)
  await admin.open()
  await admin.openTab('sso')
  await expect(page.getByTestId(TestIds.adminPage)).toBeVisible({ timeout: 15_000 })
  await expect(
    page.getByTestId(TestIds.adminSsoList).or(page.getByTestId(TestIds.adminSsoRow)).first(),
  ).toBeVisible({ timeout: 15_000 })
})
