import { expect, test, type Page } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import {
  apiJson,
  createCase,
  createProject,
  createRun,
  createSuite,
} from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { CasesPage } from '../pages/CasesPage'
import { GeneratePage } from '../pages/GeneratePage'
import { ReportsPage } from '../pages/ReportsPage'
import { SettingsPage } from '../pages/SettingsPage'

test.describe.configure({ mode: 'serial' })

async function selectProject(page: Page, projectKey: string) {
  const select = page.locator('.project-select')
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })
}

test('Generate case picker uses compact checkboxes and scrollable list', {
  tag: ['@VQA-340', '@VUE-114'],
  annotation: [
    { type: 'vulyk', description: 'VQA-340' },
    { type: 'vulyk', description: 'VUE-114' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('F8')
  const project = await createProject(request, admin.token, {
    key,
    name: `Picker ${key}`,
  })
  const suite = await createSuite(request, admin.token, project.id, 'Smoke')
  for (let i = 0; i < 12; i += 1) {
    await createCase(request, admin.token, project.id, {
      title: `Case ${i} ${key}`,
      suite_id: suite.id,
    })
  }

  await loginAsAdmin(page)
  await selectProject(page, key)
  const gen = new GeneratePage(page)
  await gen.open()

  const picker = page.getByTestId(TestIds.casePicker)
  await expect(picker).toBeVisible({ timeout: 20_000 })

  const list = page.getByTestId(TestIds.casePickerList)
  await expect(list).toBeVisible()
  const listBox = await list.boundingBox()
  expect(listBox?.height ?? 0).toBeLessThan(400)

  const checkbox = picker
    .getByTestId(TestIds.casePickerItem)
    .first()
    .locator('input[type="checkbox"]')
  await expect(checkbox).toBeVisible()
  const box = await checkbox.boundingBox()
  expect(box?.width ?? 99).toBeLessThanOrEqual(24)
  expect(box?.height ?? 99).toBeLessThanOrEqual(24)

  const folders = page.getByTestId(TestIds.casePickerFolders)
  await expect(folders.getByTestId(TestIds.casePickerAllFolders)).toBeVisible()
  await expect(folders.getByText('Smoke')).toBeVisible()

  const chips = page.getByTestId(TestIds.casePickerChips)
  await expect(chips.getByTestId(TestIds.casePickerChip)).toHaveCount(0)
  // Chips only appear after a real selection — never an empty leading chip.
  await expect(chips.locator('.chip')).toHaveCount(0)
  await checkbox.check()
  await expect(chips.getByTestId(TestIds.casePickerChip)).toHaveCount(1)
  await expect(chips.getByTestId(TestIds.casePickerChip).first()).not.toHaveText(
    /^\s*$/,
  )
})

test('Reports and Runs stay within viewport at 390px width', {
  tag: ['@VQA-341', '@VUE-115'],
  annotation: [
    { type: 'vulyk', description: 'VQA-341' },
    { type: 'vulyk', description: 'VUE-115' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('F9')
  const project = await createProject(request, admin.token, {
    key,
    name: `Mobile ${key}`,
  })
  await createCase(request, admin.token, project.id, {
    title: `Seed ${key}`,
  })
  await createRun(request, admin.token, project.id, {
    name: `Run ${key}`,
    start: true,
    include_all_cases: true,
  })

  await page.setViewportSize({ width: 390, height: 844 })
  await loginAsAdmin(page)
  await selectProject(page, key)

  const reports = new ReportsPage(page)
  await reports.open()
  await expect(page.getByTestId(TestIds.reportsPage)).toBeVisible()
  const reportsOverflow = await page.evaluate(() => {
    const el = document.scrollingElement ?? document.documentElement
    return el.scrollWidth - el.clientWidth
  })
  expect(reportsOverflow).toBeLessThanOrEqual(1)

  await page.goto('/runs')
  await expect(page.getByTestId(TestIds.runsPage)).toBeVisible({ timeout: 20_000 })
  const uploadSelect = page.getByTestId(TestIds.runsUploadRunSelect)
  if (await uploadSelect.count()) {
    const box = await uploadSelect.boundingBox()
    expect(box?.width ?? 999).toBeLessThanOrEqual(390)
  }
  const runsOverflow = await page.evaluate(() => {
    const el = document.scrollingElement ?? document.documentElement
    return el.scrollWidth - el.clientWidth
  })
  expect(runsOverflow).toBeLessThanOrEqual(1)
})

test('Case list long titles ellipsize without overlapping columns', {
  tag: ['@VQA-342', '@VUE-116'],
  annotation: [
    { type: 'vulyk', description: 'VQA-342' },
    { type: 'vulyk', description: 'VUE-116' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('F10')
  const project = await createProject(request, admin.token, {
    key,
    name: `Ellipsis ${key}`,
  })
  const longTitle =
    `Very long case title that must truncate with an ellipsis instead of overlapping the priority column ${key} ` +
    'x'.repeat(80)
  await createCase(request, admin.token, project.id, {
    title: longTitle,
    priority: 'high',
    status: 'ready',
  })

  await loginAsAdmin(page)
  await selectProject(page, key)
  const cases = new CasesPage(page)
  await cases.open()
  await expect(cases.caseListHead()).toBeVisible({ timeout: 20_000 })

  const titleCell = page.getByTestId(TestIds.caseRowTitle).first()
  await expect(titleCell).toBeVisible()
  const overflow = await titleCell.evaluate((el) => {
    const style = getComputedStyle(el)
    return {
      textOverflow: style.textOverflow,
      overflow: style.overflow,
      whiteSpace: style.whiteSpace,
      scrollWider: el.scrollWidth > el.clientWidth + 1,
    }
  })
  expect(overflow.textOverflow).toBe('ellipsis')
  expect(overflow.overflow).toMatch(/hidden/)
  expect(overflow.whiteSpace).toBe('nowrap')
  expect(overflow.scrollWider).toBe(true)

  const titleBox = await titleCell.boundingBox()
  const priority = page.getByTestId(TestIds.caseRowPriority).first()
  const priorityBox = await priority.boundingBox()
  expect(titleBox && priorityBox).toBeTruthy()
  if (titleBox && priorityBox) {
    expect(titleBox.x + titleBox.width).toBeLessThanOrEqual(priorityBox.x + 1)
  }
})

test('CI tokens: links, active project default, copy + env block, timestamps', {
  tag: ['@VQA-343', '@VUE-117'],
  annotation: [
    { type: 'vulyk', description: 'VQA-343' },
    { type: 'vulyk', description: 'VUE-117' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('F11')
  const project = await createProject(request, admin.token, {
    key,
    name: `CI tokens ${key}`,
  })

  await loginAsAdmin(page)
  await selectProject(page, key)

  await page.goto('/ci')
  await expect(page.getByTestId(TestIds.runsCiTokensLink)).toBeVisible({
    timeout: 20_000,
  })
  await page.getByTestId(TestIds.runsCiTokensLink).click()
  await expect(page.getByTestId(TestIds.settingsCiPanel)).toBeVisible({
    timeout: 15_000,
  })
  await expect(page.getByTestId(TestIds.settingsCiProjectSelect)).toHaveValue(
    project.id,
  )

  // Admin → Integrations link to CI tokens.
  await page.goto('/admin')
  await page.getByTestId(TestIds.adminTabIntegrations).click()
  await expect(
    page.getByTestId(TestIds.adminIntegrationsCiTokensLink),
  ).toBeVisible()
  await page.getByTestId(TestIds.adminIntegrationsCiTokensLink).click()
  await expect(page.getByTestId(TestIds.settingsCiPanel)).toBeVisible()

  const settings = new SettingsPage(page)
  await settings.createCiToken(`ci-${key}`)
  await expect(settings.ciTokenSecret()).toContainText(/vulyk_ci_/, {
    timeout: 15_000,
  })
  await expect(page.getByTestId(TestIds.settingsCiTokenEnvBlock)).toContainText(
    'VULYK_CI_TOKEN=',
  )
  await expect(page.getByTestId(TestIds.settingsCiTokenCopyButton)).toBeVisible()
  await expect(
    page.getByTestId(TestIds.settingsCiTokenCopyEnvButton),
  ).toBeVisible()
  await expect(page.getByTestId(TestIds.settingsCiTokenCreated).first()).toBeVisible()
  await expect(page.getByTestId(TestIds.settingsCiTokenLastUsed).first()).toBeVisible()
})

test('Run report shows Skipped tile and Pass % excludes skipped', {
  tag: ['@VQA-344', '@VUE-118'],
  annotation: [
    { type: 'vulyk', description: 'VQA-344' },
    { type: 'vulyk', description: 'VUE-118' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('F12')
  const project = await createProject(request, admin.token, {
    key,
    name: `Pass rate ${key}`,
  })
  const suite = await createSuite(request, admin.token, project.id, 'Auth')
  const a = await createCase(request, admin.token, project.id, {
    title: `Pass ${key}`,
    suite_id: suite.id,
  })
  const b = await createCase(request, admin.token, project.id, {
    title: `Fail ${key}`,
    suite_id: suite.id,
  })
  const c = await createCase(request, admin.token, project.id, {
    title: `Skip ${key}`,
    suite_id: suite.id,
  })
  const run = await createRun(request, admin.token, project.id, {
    name: `Mixed ${key}`,
    source: 'ci',
    include_all_cases: true,
    start: true,
  })
  await apiJson(
    request,
    'POST',
    `/api/v1/projects/${project.id}/runs/${run.run.id}/results`,
    {
      token: admin.token,
      data: {
        results: [
          { case_id: a.case.id, status: 'passed' },
          { case_id: b.case.id, status: 'failed' },
          { case_id: c.case.id, status: 'skipped' },
        ],
      },
    },
  )

  await loginAsAdmin(page)
  await selectProject(page, key)
  const reports = new ReportsPage(page)
  await reports.open()
  await expect(page.getByTestId(TestIds.reportsOtherLegend).first()).toBeVisible()
  await expect(page.getByTestId(TestIds.reportsSummaries)).toContainText(
    /excl\. skipped|без skipped|hors ignorés|sin omitidos|ohne übersprungen/i,
  )

  await reports.openSummary(run.run.id)
  await expect(page.getByTestId(TestIds.reportsRunKpiSkipped)).toContainText('1')
  await expect(page.getByTestId(TestIds.reportsRunKpiPassRate)).toContainText(
    '50.0%',
  )
  await expect(page.getByTestId(TestIds.reportsRunPassRateHint)).toBeVisible()
})
