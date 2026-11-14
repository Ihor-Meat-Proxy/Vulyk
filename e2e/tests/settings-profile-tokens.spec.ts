import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { SettingsPage } from '../pages/SettingsPage'
import { openUserMenu } from '../fixtures/header'

test.describe.configure({ mode: 'serial' })

test('settings profile save + personal API token create', {
  tag: ['@VQA-315', '@VUE-93'], annotation: [{ type: 'vulyk', description: 'VQA-315' }, { type: 'vulyk', description: 'VUE-93' }],
}, async ({ page, request }) => {
  await ensureAdminSession(request)
  await loginAsAdmin(page)
  const settings = new SettingsPage(page)
  await settings.open()

  await expect(page.getByTestId(TestIds.settingsDisplayNameInput)).toBeVisible()
  const name = `E2E Admin ${uniqueKey('NM').slice(-4)}`
  await page.getByTestId(TestIds.settingsDisplayNameInput).fill(name)
  await page.getByTestId(TestIds.settingsSaveProfileButton).click()
  await expect(page.getByTestId(TestIds.settingsMessage)).toBeVisible({
    timeout: 15_000,
  })

  const tokenName = `pat-${uniqueKey('TK').toLowerCase()}`
  await page.getByTestId(TestIds.settingsTokenNameInput).fill(tokenName)
  await page.getByTestId(TestIds.settingsCreateTokenButton).click()
  await expect(page.getByTestId(TestIds.settingsTokenSecret)).toBeVisible({
    timeout: 15_000,
  })
  const secret = await page.getByTestId(TestIds.settingsTokenSecret).innerText()
  expect(secret).toMatch(/vulyk_at_/)
  await expect(page.getByTestId(TestIds.settingsTokenList)).toContainText(
    tokenName,
  )
})

test('settings change-password rejects short password', {
  tag: ['@VQA-316', '@VUE-91'], annotation: [{ type: 'vulyk', description: 'VQA-316' }, { type: 'vulyk', description: 'VUE-91' }],
}, async ({ page, request }) => {
  await ensureAdminSession(request)
  await loginAsAdmin(page)
  const settings = new SettingsPage(page)
  await settings.open()

  await page.getByTestId(TestIds.settingsCurrentPasswordInput).fill(
    process.env.VULYK_ADMIN_PASSWORD ?? 'admin12345678901',
  )
  await page.getByTestId(TestIds.settingsNewPasswordInput).fill('short')
  await page.getByTestId(TestIds.settingsConfirmPasswordInput).fill('short')
  await expect(page.getByTestId(TestIds.settingsChangePasswordButton)).toBeDisabled()
  await expect(page.getByTestId(TestIds.passwordRequirements)).toBeVisible()
})

test('settings notify prefs toggles persist after reload', {
  tag: ['@VQA-317', '@VUE-92'], annotation: [{ type: 'vulyk', description: 'VQA-317' }, { type: 'vulyk', description: 'VUE-92' }],
}, async ({ page, request }) => {
  await ensureAdminSession(request)
  await loginAsAdmin(page)
  const settings = new SettingsPage(page)
  await settings.open()

  const emailBox = page.getByTestId(TestIds.settingsNotifyEmailCheckbox)
  await expect(emailBox).toBeVisible()
  const before = await emailBox.isChecked()
  if (before) await emailBox.uncheck()
  else await emailBox.check()
  await page.getByTestId(TestIds.settingsSaveProfileButton).click()
  await expect(page.getByTestId(TestIds.settingsMessage)).toBeVisible({
    timeout: 15_000,
  })
  await page.reload()
  await openUserMenu(page)
  await page.getByTestId(TestIds.headerGearSettings).click()
  await expect(page.getByTestId(TestIds.settingsNotifyEmailCheckbox)).toBeChecked({
    checked: !before,
  })
  // restore
  const box = page.getByTestId(TestIds.settingsNotifyEmailCheckbox)
  if (before) await box.check()
  else await box.uncheck()
  await page.getByTestId(TestIds.settingsSaveProfileButton).click()
})
