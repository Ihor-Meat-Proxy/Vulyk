import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import {
  createCase,
  createProject,
  createSuite,
} from '../fixtures/api'
import {
  createUserWithRole,
  ensureAdminSession,
  loginAs,
  loginAsAdmin,
} from '../fixtures/auth'
import { API_URL, uniqueKey } from '../fixtures/env'

test.describe.configure({ mode: 'serial' })

test('role matrix: mutate case allowed or denied per role', {
  tag: ['@VQA-298', '@VUE-75'], annotation: [{ type: 'vulyk', description: 'VQA-298' }, { type: 'vulyk', description: 'VUE-75' }],
}, async ({ request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('RM')
  const project = await createProject(request, admin.token, {
    key,
    name: `Roles ${key}`,
  })
  await createSuite(request, admin.token, project.id, 'RBAC')
  await createCase(request, admin.token, project.id, {
    title: `Seed ${key}`,
    status: 'ready',
    priority: 'medium',
  })

  const expectCreate = async (role: string, allow: boolean) => {
    const user = await createUserWithRole(
      request,
      admin.token,
      project.id,
      role,
    )
    const res = await request.post(
      `${API_URL}/api/v1/projects/${project.id}/cases`,
      {
        headers: {
          Authorization: `Bearer ${user.session.token}`,
          'Content-Type': 'application/json',
        },
        data: {
          title: `${role} case ${key}`,
          status: 'draft',
          priority: 'low',
        },
      },
    )
    if (allow) expect(res.ok(), `${role} should create`).toBeTruthy()
    else expect(res.status(), `${role} denied`).toBe(403)
  }

  // Developer has no CreateEditCases; viewer none; architect/TAE/admin can create.
  await expectCreate('viewer', false)
  await expectCreate('developer', false)
  await expectCreate('test_architect', true)
  await expectCreate('test_automation_engineer', true)
  await expectCreate('admin', true)
})

test('developer can open cases UI but not admin', {
  tag: ['@VQA-299', '@VUE-74'], annotation: [{ type: 'vulyk', description: 'VQA-299' }, { type: 'vulyk', description: 'VUE-74' }],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('TS')
  const project = await createProject(request, admin.token, {
    key,
    name: `Dev UI ${key}`,
  })
  const user = await createUserWithRole(
    request,
    admin.token,
    project.id,
    'developer',
  )
  await loginAs(page, user.email, user.password)
  await expect(page.getByTestId(TestIds.appShell)).toBeVisible()
  await expect(page.getByTestId(TestIds.navLinkCases)).toBeVisible()

  const gear = page.getByTestId(TestIds.headerGearMenu)
  await gear.click()
  await expect(page.getByTestId(TestIds.headerGearAdmin)).toHaveCount(0)
})

test('viewer cannot open import page', {
  tag: ['@VQA-300', '@VUE-76'],
  annotation: [
    { type: 'vulyk', description: 'VQA-300' },
    { type: 'vulyk', description: 'VUE-76' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('VW')
  const project = await createProject(request, admin.token, {
    key,
    name: `Viewer UI ${key}`,
  })
  const user = await createUserWithRole(
    request,
    admin.token,
    project.id,
    'viewer',
  )
  await loginAs(page, user.email, user.password)
  await expect(page.getByTestId(TestIds.navLinkImport)).toHaveCount(0)
  // Soft UI deny: import nav hidden; mutating import API stays 403 (covered in rbac.spec).
  void loginAsAdmin
})
