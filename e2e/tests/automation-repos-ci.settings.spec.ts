import { expect, test } from '@playwright/test'
import { createProject } from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { SettingsPage } from '../pages/SettingsPage'

test.describe.configure({ mode: 'serial' })

test('settings: automation repo + project CI token CRUD', {
  tag: ['@VQA-239', '@VUE-15'], annotation: [{ type: 'vulyk', description: 'VQA-239' }, { type: 'vulyk', description: 'VUE-15' }],
}, async ({
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
    projectId: project.id,
  })
  await settings.submitAddRepo()
  // Assert the uniquely named row — never global table length (leftovers flake).
  const createdRow = settings.repoRows().filter({ hasText: repoName })
  await expect(createdRow).toHaveCount(1, { timeout: 15_000 })
  await expect(createdRow).toBeVisible()

  await settings.expectCiTokensHeading()
  await settings.selectCiProject(project.id)
  await settings.createCiToken(`ci-${key}`)

  await expect(settings.ciTokenSecret()).toContainText(/vulyk_ci_/, {
    timeout: 15_000,
  })
})
