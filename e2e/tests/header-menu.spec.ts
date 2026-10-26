import { expect, test } from '@playwright/test'
import {
  createProject,
  createSuite,
} from '../fixtures/api'
import {
  createUserWithRole,
  ensureAdminSession,
  loginAs,
  loginAsAdmin,
  logout,
} from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { openUserMenu, userMenuItem } from '../fixtures/header'

test.describe.configure({ mode: 'serial' })

test('gear menu: appearance, admin for admin only, logout', async ({
  page,
  request,
}) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('HM')
  const project = await createProject(request, admin.token, {
    key,
    name: `Header ${key}`,
  })
  await createSuite(request, admin.token, project.id, 'Header Suite')
  const viewer = await createUserWithRole(
    request,
    admin.token,
    project.id,
    'viewer',
  )

  await loginAsAdmin(page)

  const primary = page.getByRole('navigation', { name: 'Primary' })
  await expect(primary.getByRole('link', { name: /Admin|Адмін/i })).toHaveCount(
    0,
  )
  await expect(
    primary.getByRole('link', { name: /Appearance|Вигляд/i }),
  ).toHaveCount(0)

  await openUserMenu(page)
  await expect(page.locator('.user-avatar')).toBeVisible()
  await expect(page.locator('.mode-badge')).toBeVisible()
  await expect(
    userMenuItem(
      page,
      /Admin settings|Адмін-налаштування|Paramètres d.administration|Ajustes de administración|Admin-Einstellungen/i,
    ),
  ).toBeVisible()

  await userMenuItem(
    page,
    /Appearance|Вигляд|Apparence|Apariencia|Erscheinungsbild/i,
  ).click()
  await expect(
    page.getByRole('heading', {
      name: /Appearance|Вигляд|Apparence|Apariencia|Erscheinungsbild/i,
    }),
  ).toBeVisible({ timeout: 15_000 })

  await openUserMenu(page)
  await userMenuItem(
    page,
    /Admin settings|Адмін-налаштування|Paramètres d.administration|Ajustes de administración|Admin-Einstellungen/i,
  ).click()
  await expect(
    page.getByRole('heading', { name: /Admin|Адмін|Administration/i }),
  ).toBeVisible({ timeout: 15_000 })

  await openUserMenu(page)
  await userMenuItem(
    page,
    /My settings|Мої налаштування|Mes paramètres|Mis ajustes|Meine Einstellungen/i,
  ).click()
  await expect(
    page.getByRole('heading', {
      name: /My settings|Мої налаштування|Mes paramètres|Mis ajustes|Meine Einstellungen/i,
    }),
  ).toBeVisible({ timeout: 15_000 })

  await logout(page)

  await loginAs(page, viewer.email, viewer.password)
  await openUserMenu(page)
  await expect(
    page.getByRole('menuitem', {
      name: /Admin settings|Адмін-налаштування|Paramètres d.administration|Ajustes de administración|Admin-Einstellungen/i,
    }),
  ).toHaveCount(0)
  await expect(
    userMenuItem(
      page,
      /Appearance|Вигляд|Apparence|Apariencia|Erscheinungsbild/i,
    ),
  ).toBeVisible()
})
