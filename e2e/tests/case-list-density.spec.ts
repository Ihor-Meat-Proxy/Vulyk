import { expect, test, type Page } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import { createCase, createProject } from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { CasesPage } from '../pages/CasesPage'

test.describe.configure({ mode: 'serial' })

async function selectProject(page: Page, projectKey: string) {
  const select = page.locator('.project-select')
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })
}

test('case list sticky header remains visible while scrolling virtual rows', {
  tag: ['@VQA-360', '@VUE-222'],
  annotation: [
    { type: 'vulyk', description: 'VQA-360' },
    { type: 'vulyk', description: 'VUE-222' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('CLD')
  const project = await createProject(request, admin.token, {
    key,
    name: `Density ${key}`,
  })
  for (let i = 0; i < 40; i += 1) {
    await createCase(request, admin.token, project.id, {
      title: `Row ${String(i).padStart(2, '0')} ${key}`,
      status: 'ready',
      priority: 'medium',
    })
  }

  await loginAsAdmin(page)
  await selectProject(page, key)
  const cases = new CasesPage(page)
  await cases.open()

  const head = cases.caseListHead()
  await expect(head).toBeVisible({ timeout: 20_000 })
  const scroll = page.getByTestId(TestIds.caseListVirtualScroll)
  await expect(scroll).toBeVisible()

  const before = await head.boundingBox()
  expect(before).toBeTruthy()

  await scroll.evaluate((el) => {
    el.scrollTop = el.scrollHeight
  })
  await expect
    .poll(async () => scroll.evaluate((el) => el.scrollTop))
    .toBeGreaterThan(200)

  await expect(head).toBeVisible()
  const after = await head.boundingBox()
  expect(after).toBeTruthy()
  // Sticky: header stays pinned near the top of the scrollport.
  expect(Math.abs((after?.y ?? 0) - (before?.y ?? 0))).toBeLessThan(8)
})

test('column visibility toggle does not refetch cases?limit=200', {
  tag: ['@VQA-361', '@VUE-223'],
  annotation: [
    { type: 'vulyk', description: 'VQA-361' },
    { type: 'vulyk', description: 'VUE-223' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('CLP')
  const project = await createProject(request, admin.token, {
    key,
    name: `Prefs ${key}`,
  })
  await createCase(request, admin.token, project.id, {
    title: `Pref case ${key}`,
    status: 'ready',
  })

  await loginAsAdmin(page)
  await selectProject(page, key)
  const cases = new CasesPage(page)
  await cases.open()
  await expect(cases.caseListHead()).toBeVisible({ timeout: 20_000 })

  const listUrl = (url: string) =>
    url.includes(`/projects/${project.id}/cases`) &&
    url.includes('limit=200') &&
    !url.includes('limit=1')

  // Wait until the initial list fetch has settled.
  await expect
    .poll(async () => cases.caseCount().textContent())
    .toMatch(/1/)

  const listHits: string[] = []
  const onRequest = (req: import('@playwright/test').Request) => {
    if (req.method() === 'GET' && listUrl(req.url())) {
      listHits.push(req.url())
    }
  }
  page.on('request', onRequest)

  await cases.openColumnsSettings()
  await cases.toggleColumn('priority', false)
  await expect(
    cases.caseListHead().getByTestId(TestIds.caseListColPriority),
  ).toHaveCount(0)
  await cases.toggleColumn('automated', true)
  await expect(
    cases.caseListHead().getByTestId(TestIds.caseListColAutomated),
  ).toBeVisible()
  await cases.moveColumn('key', 'right')

  // Give any accidental refetch a moment to fire.
  await page.waitForTimeout(600)
  page.off('request', onRequest)

  expect(
    listHits,
    `column prefs must not refetch cases?limit=200; got: ${listHits.join(' | ')}`,
  ).toEqual([])
})

test('virtualized case list reveals a known case after scroll', {
  tag: ['@VQA-362', '@VUE-224'],
  annotation: [
    { type: 'vulyk', description: 'VQA-362' },
    { type: 'vulyk', description: 'VUE-224' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('CLV')
  const project = await createProject(request, admin.token, {
    key,
    name: `Virtual ${key}`,
  })
  const titles: string[] = []
  for (let i = 0; i < 50; i += 1) {
    const title = `Virt ${String(i).padStart(2, '0')} ${key}`
    titles.push(title)
    await createCase(request, admin.token, project.id, {
      title,
      status: 'ready',
      priority: i % 2 === 0 ? 'high' : 'low',
    })
  }
  const targetTitle = titles[titles.length - 1]

  await loginAsAdmin(page)
  await selectProject(page, key)
  const cases = new CasesPage(page)
  await cases.open()
  await expect(cases.caseListHead()).toBeVisible({ timeout: 20_000 })

  const scroll = page.getByTestId(TestIds.caseListVirtualScroll)
  await expect(scroll).toBeVisible()

  // Target is near the bottom — should not be in the first paint window.
  const firstPaint = cases
    .listPanel()
    .getByTestId(TestIds.caseRowTitle)
    .filter({ hasText: targetTitle })
  await expect(firstPaint).toHaveCount(0)

  await scroll.evaluate((el) => {
    el.scrollTop = el.scrollHeight
  })

  await expect(
    cases.listPanel().getByTestId(TestIds.caseRowTitle).filter({ hasText: targetTitle }),
  ).toBeVisible({ timeout: 10_000 })
})
