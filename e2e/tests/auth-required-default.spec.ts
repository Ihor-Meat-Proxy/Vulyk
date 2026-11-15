import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { API_URL } from '../fixtures/env'
import { openUserMenu } from '../fixtures/header'

test.describe.configure({ mode: 'serial' })

test.beforeAll(async ({ request }) => {
  await ensureAdminSession(request)
})

test('auth required by default: anonymous API denied and gear shows Auth required', {
  tag: ['@VQA-337', '@VUE-111'],
  annotation: [
    { type: 'vulyk', description: 'VQA-337' },
    { type: 'vulyk', description: 'VUE-111' },
  ],
}, async ({ page, request }) => {
  const anon = await request.get(`${API_URL}/api/v1/projects`)
  expect(anon.status()).toBe(401)

  const status = await request.get(`${API_URL}/api/v1/auth/status`)
  expect(status.ok()).toBeTruthy()
  const body = (await status.json()) as { auth_required?: boolean }
  expect(body.auth_required).toBe(true)

  await loginAsAdmin(page)
  await openUserMenu(page)
  const mode = page.getByTestId(TestIds.headerGearAuthMode)
  await expect(mode).toBeVisible()
  await expect(mode).toHaveText(/Auth required|Потрібна автентифікація|Authentification requise|Autenticación requerida|Anmeldung erforderlich/i)
  await expect(mode).not.toHaveText(/Open mode|Відкритий режим|Mode ouvert|Modo abierto|Offener Modus/i)
})
