import { expect, test } from '@playwright/test'
import { createCase, createProject } from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { CasesPage } from '../pages/CasesPage'

test.describe.configure({ mode: 'serial' })

let projectKey = ''
let projectId = ''
let adminToken = ''

test.beforeAll(async ({ request }) => {
  const admin = await ensureAdminSession(request)
  adminToken = admin.token
  projectKey = uniqueKey('CL')
  const project = await createProject(request, admin.token, {
    key: projectKey,
    name: `Columns ${projectKey}`,
  })
  projectId = project.id
  await createCase(request, admin.token, projectId, {
    title: `Alpha ${projectKey}`,
    status: 'ready',
    priority: 'medium',
  })
  await createCase(request, admin.token, projectId, {
    title: `Zebra ${projectKey}`,
    status: 'ready',
    priority: 'high',
    external_key: 'e2e:case-list-columns.spec.ts',
  })
})

async function selectProject(page: import('@playwright/test').Page) {
  const select = page.locator('.project-select')
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })
}

test('customize columns: hide priority, show automated, keyboard reorder, sort, reset', async ({
  page,
}) => {
  await loginAsAdmin(page)
  await selectProject(page)

  const cases = new CasesPage(page)
  await cases.open()
  await expect(cases.caseListHead()).toBeVisible({ timeout: 20_000 })
  await expect(cases.caseListHead()).toContainText(/Key|Ключ|Clé|Clave|Schlüssel/i)
  await expect(cases.caseListHead()).toContainText(/Title|Назва|Titre|Título|Titel/i)
  await expect(cases.caseListHead()).toContainText(/Priority|Пріоритет|Priorité|Prioridad|Priorität/i)

  await cases.openColumnsSettings()
  const titleBox = cases
    .columnsDialog()
    .getByRole('checkbox', { name: /Show column Title|Показати стовпець Назва/i })
  await expect(titleBox).toBeChecked()
  await expect(titleBox).toBeDisabled()

  await cases.toggleColumn(/Priority|Пріоритет/i, false)
  await cases.toggleColumn(/Automated|Автоматизовано/i, true)
  await expect(cases.caseListHead()).not.toContainText(/Priority|Пріоритет/i)
  await expect(cases.caseListHead()).toContainText(/Automated|Автоматизовано/i)
  await expect(
    cases.listPanel().getByText(/Yes|Так/i).first(),
  ).toBeVisible()

  await cases.moveColumn(/Key|Ключ/i, 'right')
  await cases.columnsButton().click()
  await expect(cases.columnsDialog()).toBeHidden()

  const sortReq = page.waitForRequest(
    (r) =>
      r.url().includes(`/projects/${projectId}/cases`) &&
      r.url().includes('sort=title'),
  )
  await cases
    .caseListHead()
    .getByRole('button', { name: /Sort by Title|Сортувати за Назва/i })
    .click()
  await sortReq

  await cases.openColumnsSettings()
  await cases.resetColumns()
  await expect(cases.caseListHead()).toContainText(/Priority|Пріоритет/i)
  await expect(cases.caseListHead()).not.toContainText(
    /Automated|Автоматизовано/i,
  )
})

test('column preferences API persists per project', async ({ request }) => {
  const body = {
    columns: [
      { id: 'title', visible: true },
      { id: 'key', visible: true },
      { id: 'automated', visible: true },
      { id: 'priority', visible: false },
      { id: 'status', visible: true },
    ],
    sort: 'title',
    order: 'asc',
  }
  const put = await request.put(
    `/api/v1/projects/${projectId}/preferences/case-list-columns`,
    {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: body,
    },
  )
  expect(put.ok()).toBeTruthy()
  const saved = await put.json()
  expect(saved.sort).toBe('title')
  expect(
    saved.columns.some(
      (c: { id: string; visible: boolean }) =>
        c.id === 'automated' && c.visible === true,
    ),
  ).toBeTruthy()

  const get = await request.get(
    `/api/v1/projects/${projectId}/preferences/case-list-columns`,
    { headers: { Authorization: `Bearer ${adminToken}` } },
  )
  expect(get.ok()).toBeTruthy()
  const loaded = await get.json()
  expect(loaded.sort).toBe('title')

  const reset = await request.delete(
    `/api/v1/projects/${projectId}/preferences/case-list-columns`,
    { headers: { Authorization: `Bearer ${adminToken}` } },
  )
  expect(reset.ok()).toBeTruthy()
  const defaults = await reset.json()
  expect(defaults.sort == null).toBeTruthy()
})
