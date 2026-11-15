import { expect, test, type Page } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import {
  apiJson,
  createCase,
  createProject,
} from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin, logout } from '../fixtures/auth'
import { ADMIN_EMAIL, ADMIN_PASSWORD, uniqueKey } from '../fixtures/env'
import { AdminPage } from '../pages/AdminPage'
import { SettingsPage } from '../pages/SettingsPage'

test.describe.configure({ mode: 'serial' })

async function selectProject(page: Page, projectKey: string) {
  const select = page.locator('.project-select')
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })
}

test('unknown route shows 404 page with back link', {
  tag: ['@VQA-345', '@VUE-207'],
  annotation: [
    { type: 'vulyk', description: 'VQA-345' },
    { type: 'vulyk', description: 'VUE-207' },
  ],
}, async ({ page, request }) => {
  await ensureAdminSession(request)
  await loginAsAdmin(page)
  await page.goto('/this-route-does-not-exist-f17')
  await expect(page.getByTestId(TestIds.notFoundPage)).toBeVisible({
    timeout: 20_000,
  })
  await expect(page.getByTestId(TestIds.notFoundTitle)).toBeVisible()
  await page.getByTestId(TestIds.notFoundBackLink).click()
  await expect(page.getByTestId(TestIds.caseListPanel)).toBeVisible({
    timeout: 20_000,
  })
})

test('bad password login shows friendly message', {
  tag: ['@VQA-346', '@VUE-208'],
  annotation: [
    { type: 'vulyk', description: 'VQA-346' },
    { type: 'vulyk', description: 'VUE-208' },
  ],
}, async ({ page }) => {
  await page.goto('/login')
  if (await page.getByTestId(TestIds.appShell).isVisible()) {
    await logout(page)
  }
  await expect(page.getByTestId(TestIds.loginPanel)).toBeVisible({
    timeout: 20_000,
  })
  await page.getByTestId(TestIds.loginEmailInput).fill(ADMIN_EMAIL)
  await page.getByTestId(TestIds.loginPasswordInput).fill('definitely-wrong-password!!')
  await page.getByTestId(TestIds.loginSubmitButton).click()
  const err = page.getByTestId(TestIds.loginError)
  await expect(err).toBeVisible({ timeout: 15_000 })
  await expect(err).not.toContainText(/unauthorized|bad request|\{/i)
  await expect(err).toContainText(
    /incorrect email or password|неправильн|incorrect|incorrectos|falsch|sign-in failed|не вдалося|échec|no se pudo|fehlgeschlagen/i,
  )
  expect(ADMIN_PASSWORD.length).toBeGreaterThan(8)
})

test('empty case executions show dash pass rate and 0 of 0 pager', {
  tag: ['@VQA-347', '@VUE-209'],
  annotation: [
    { type: 'vulyk', description: 'VQA-347' },
    { type: 'vulyk', description: 'VUE-209' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('F15')
  const project = await createProject(request, admin.token, {
    key,
    name: `Empty exec ${key}`,
  })
  const created = await createCase(request, admin.token, project.id, {
    title: `No runs ${key}`,
  })

  await loginAsAdmin(page)
  await selectProject(page, key)
  await page.goto(`/cases/${created.case.id}`)
  await page.getByRole('tab', { name: /Executions|Виконання|Ausführungen|Ejecuciones|Exécutions/i }).click()
  await expect(page.getByTestId(TestIds.caseDetailExecutionsPassRate)).toHaveText(
    '—',
    { timeout: 20_000 },
  )
  await expect(page.getByTestId(TestIds.caseDetailExecutionsPager)).toContainText(
    /0\s+(of|von|de|з)\s+0/i,
  )
})

test('CI lifecycle run is labeled CI run on case executions tab', {
  tag: ['@VQA-348', '@VUE-210'],
  annotation: [
    { type: 'vulyk', description: 'VQA-348' },
    { type: 'vulyk', description: 'VUE-210' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('F13')
  const project = await createProject(request, admin.token, {
    key,
    name: `CI label ${key}`,
  })
  const created = await createCase(request, admin.token, project.id, {
    title: `CI case ${key}`,
  })
  const opened = await apiJson<{
    run: { id: string; lifecycle: string }
  }>(request, 'POST', `/api/v1/projects/${project.id}/ci/runs`, {
    token: admin.token,
    data: {
      provider: 'github',
      repo: `acme/${key}`,
      branch: 'main',
      commit: 'abc1234',
      suite_name: `Suite ${key}`,
      external_ref: `ref-${key}`,
    },
  })
  expect(opened.run.lifecycle).toBe('ci')
  await apiJson(
    request,
    'POST',
    `/api/v1/projects/${project.id}/ci/runs/${opened.run.id}/results`,
    {
      token: admin.token,
      data: {
        results: [
          {
            case_key: created.case.key,
            status: 'passed',
            external_id: 'spec::ci',
            duration_ms: 12,
          },
        ],
      },
    },
  )

  const history = await apiJson<{
    items: Array<{ plan_type: string }>
  }>(
    request,
    'GET',
    `/api/v1/projects/${project.id}/cases/${created.case.id}/executions?limit=10&offset=0`,
    { token: admin.token },
  )
  expect(history.items.some((i) => i.plan_type === 'ci')).toBeTruthy()
  expect(history.items.every((i) => i.plan_type !== 'iteration')).toBeTruthy()

  await loginAsAdmin(page)
  await selectProject(page, key)
  await page.goto(`/cases/${created.case.id}`)
  await page.getByRole('tab', { name: /Executions|Виконання|Ausführungen|Ejecuciones|Exécutions/i }).click()
  await expect(page.locator('table.table')).toBeVisible({ timeout: 20_000 })
  await expect(page.locator('table.table')).toContainText(
    /CI run|CI-прогін|Exécution CI|Ejecución CI|CI-Lauf/i,
  )
})

test('Admin opens on Users tab for system admin', {
  tag: ['@VQA-349', '@VUE-211'],
  annotation: [
    { type: 'vulyk', description: 'VQA-349' },
    { type: 'vulyk', description: 'VUE-211' },
  ],
}, async ({ page, request }) => {
  await ensureAdminSession(request)
  await loginAsAdmin(page)
  const admin = new AdminPage(page)
  await admin.open()
  await expect(page.getByTestId(TestIds.adminTabUsers)).toHaveAttribute(
    'aria-selected',
    'true',
  )
  await expect(page.getByTestId(TestIds.adminUserList)).toBeVisible()
})

test('settings timezone normalizes Europe/Kiev to Europe/Kyiv', {
  tag: ['@VQA-350', '@VUE-212'],
  annotation: [
    { type: 'vulyk', description: 'VQA-350' },
    { type: 'vulyk', description: 'VUE-212' },
  ],
}, async ({ page, request }) => {
  await ensureAdminSession(request)
  await loginAsAdmin(page)
  const settings = new SettingsPage(page)
  await settings.open()
  const input = page.getByTestId(TestIds.settingsTimezoneInput)
  await expect(input).toBeVisible({ timeout: 20_000 })
  await input.fill('Europe/Kiev')
  await input.blur()
  await expect(input).toHaveValue('Europe/Kyiv')
})

test('Runs JUnit upload shows hint while disabled; header icons are tappable size', {
  tag: ['@VQA-351', '@VUE-213'],
  annotation: [
    { type: 'vulyk', description: 'VQA-351' },
    { type: 'vulyk', description: 'VUE-213' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('F22')
  await createProject(request, admin.token, { key, name: `Runs ${key}` })
  await loginAsAdmin(page)
  await selectProject(page, key)
  await page.goto('/runs')
  await expect(page.getByTestId(TestIds.runsUploadDisabledHint)).toBeVisible({
    timeout: 20_000,
  })
  await expect(page.getByTestId(TestIds.runsUploadFileInput)).toBeDisabled()

  const lang = page.getByTestId(TestIds.headerLanguageMenu)
  await expect(lang).toBeVisible()
  const box = await lang.boundingBox()
  expect(box?.width ?? 0).toBeGreaterThanOrEqual(36)
  expect(box?.height ?? 0).toBeGreaterThanOrEqual(36)
})

test('automation-repo returns 200 null when none configured', {
  tag: ['@VQA-352', '@VUE-214'],
  annotation: [
    { type: 'vulyk', description: 'VQA-352' },
    { type: 'vulyk', description: 'VUE-214' },
  ],
}, async ({ request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('F21')
  const project = await createProject(request, admin.token, {
    key,
    name: `No repo ${key}`,
  })
  const res = await request.get(
    `/api/v1/projects/${project.id}/automation-repo`,
    { headers: { Authorization: `Bearer ${admin.token}` } },
  )
  expect(res.status()).toBe(200)
  expect(await res.text()).toBe('null')
})

test('case edit title input is wide enough; copy link uses sans font', {
  tag: ['@VQA-353', '@VUE-215'],
  annotation: [
    { type: 'vulyk', description: 'VQA-353' },
    { type: 'vulyk', description: 'VUE-215' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('F20')
  const project = await createProject(request, admin.token, {
    key,
    name: `Title ${key}`,
  })
  const created = await createCase(request, admin.token, project.id, {
    title: `Editable title ${key}`,
  })
  await loginAsAdmin(page)
  await selectProject(page, key)
  await page.goto(`/cases/${created.case.id}`)
  await page.getByTestId(TestIds.caseDetailEditButton).click()
  const titleInput = page.getByTestId(TestIds.caseDetailTitleInput)
  await expect(titleInput).toBeVisible()
  const box = await titleInput.boundingBox()
  expect(box?.width ?? 0).toBeGreaterThanOrEqual(240)

  const copy = page.locator('.case-key-copy-link').first()
  if (await copy.isVisible()) {
    const font = await copy.evaluate((el) => getComputedStyle(el).fontFamily)
    expect(font.toLowerCase()).not.toMatch(/times|georgia|serif/)
  }

  await page.setViewportSize({ width: 390, height: 844 })
  await expect(page.getByTestId(TestIds.caseDetailBackLink)).toBeVisible()
  await expect(
    page.getByTestId(TestIds.caseDetailBackLink).locator('.shortcut-hint'),
  ).toBeHidden()
})
