import { expect, test } from '@playwright/test'
import {
  createCase,
  createProject,
  createSuite,
} from '../fixtures/api'
import {
  ADMIN_EMAIL,
  ADMIN_PASSWORD,
  createUserWithRole,
  ensureAdminSession,
  loginAs,
  loginAsAdmin,
} from '../fixtures/auth'
import { API_URL, uniqueKey } from '../fixtures/env'

test.describe.configure({ mode: 'serial' })

test('viewer is denied mutating APIs and hides write UI', {
  tag: '@VQA-287', annotation: { type: 'vulyk', description: 'VQA-287' },
}, async ({
  page,
  request,
}) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('RB')
  const project = await createProject(request, admin.token, {
    key,
    name: `RBAC ${key}`,
  })
  await createSuite(request, admin.token, project.id, 'RBAC Suite')
  await createCase(request, admin.token, project.id, {
    key: `${key}-1`,
    title: `Viewer case ${key}`,
    priority: 'medium',
    status: 'ready',
    steps: [{ action: 'Check', expected_result: 'OK' }],
  })

  const viewer = await createUserWithRole(
    request,
    admin.token,
    project.id,
    'viewer',
  )

  const denied = await request.post(
    `${API_URL}/api/v1/projects/${project.id}/cases`,
    {
      headers: {
        Authorization: `Bearer ${viewer.session.token}`,
        'Content-Type': 'application/json',
      },
      data: {
        key: `${key}-X`,
        title: 'Should fail',
        priority: 'low',
        status: 'draft',
      },
    },
  )
  expect(denied.status()).toBe(403)

  const list = await request.get(
    `${API_URL}/api/v1/projects/${project.id}/cases`,
    { headers: { Authorization: `Bearer ${viewer.session.token}` } },
  )
  expect(list.ok()).toBeTruthy()

  await loginAs(page, viewer.email, viewer.password)

  // Prefer selecting our project when the select is present
  const select = page.locator('.project-select')
  if (await select.isVisible()) {
    const options = await select.locator('option').allTextContents()
    const match = options.findIndex((o) => o.includes(key))
    if (match >= 0) await select.selectOption({ index: match })
  }

  await expect(
    page.getByLabel(
      /New case title|Назва нового кейсу|Titre du nouveau cas|Título del nuevo caso|Titel des neuen Falls/i,
    ),
  ).toHaveCount(0)
  await expect(page.getByRole('button', { name: /Create case|Створити кейс/i })).toHaveCount(0)

  const nav = page.getByRole('navigation', { name: 'Primary' })
  await expect(nav.getByRole('link', { name: /Import|Імпорт/i })).toHaveCount(0)
  await expect(nav.getByRole('link', { name: /Reports|Звіти/i })).toBeVisible()
})

test('admin sees admin nav and can list users', {
  tag: '@VQA-288', annotation: { type: 'vulyk', description: 'VQA-288' },
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const users = await request.get(`${API_URL}/api/v1/users`, {
    headers: { Authorization: `Bearer ${admin.token}` },
  })
  expect(users.ok()).toBeTruthy()

  await loginAsAdmin(page)
  await expect(
    page
      .getByRole('navigation', { name: 'Primary' })
      .getByRole('link', { name: /Admin|Адмін/i }),
  ).toHaveCount(0)
  await page
    .getByRole('button', {
      name: /User menu|Меню користувача|Menu utilisateur|Menú de usuario|Benutzermenü/i,
    })
    .click()
  await expect(
    page.getByRole('menuitem', {
      name: /Admin settings|Адмін-налаштування|Paramètres d.administration|Ajustes de administración|Admin-Einstellungen/i,
    }),
  ).toBeVisible()
  expect(ADMIN_EMAIL).toBeTruthy()
  expect(ADMIN_PASSWORD.length).toBeGreaterThanOrEqual(16)
})
