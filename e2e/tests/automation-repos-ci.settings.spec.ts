import { expect, test } from '@playwright/test'
import { createProject } from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { SettingsPage } from '../pages/SettingsPage'

test.describe.configure({ mode: 'serial' })

test('settings: automation repo + project CI token CRUD', async ({
  page,
  request,
}) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('AR')
  const project = await createProject(request, admin.token, {
    key,
    name: `AutoRepo ${key}`,
  })

  await loginAsAdmin(page)
  const settings = new SettingsPage(page)
  await settings.open()
  await settings.expectAutomationHeading()

  const repoName = `e2e-repo-${key}`
  await settings.fillNewRepo({
    name: repoName,
    remoteUrl: 'https://github.com/example/e2e-tests.git',
    projectKey: key,
  })
  await settings.submitAddRepo()
  await expect(page.getByText(repoName)).toBeVisible({ timeout: 15_000 })

  await settings.expectCiTokensHeading()
  await settings.selectCiProject(project.id, key)
  await settings.createCiToken(`ci-${key}`)

  await expect(page.locator('code')).toContainText(/vulyk_ci_/, {
    timeout: 15_000,
  })
})
