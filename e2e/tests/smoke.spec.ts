import { expect, test } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { TestIds } from '../../frontend/src/testing/testIds'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'

/**
 * Kept as a short end-to-end happy path. Focused coverage lives in sibling specs.
 */
const __dirname = path.dirname(fileURLToPath(import.meta.url))
const fixtures = path.join(__dirname, '..', 'fixtures')

test.describe.configure({ mode: 'serial' })

test.beforeAll(async ({ request }) => {
  await ensureAdminSession(request)
})

test('smoke: login → case → run → junit → import → report', {
  tag: '@VQA-301', annotation: { type: 'vulyk', description: 'VQA-301' },
}, async ({ page }) => {
  await loginAsAdmin(page)
  await expect(page.getByTestId(TestIds.appShell)).toBeVisible()
  await expect(page.getByTestId(TestIds.navLinkCases)).toBeVisible()

  await expect(page.getByTestId(TestIds.headerProjectSelect)).toBeVisible()
  await expect
    .poll(
      async () =>
        page.getByTestId(TestIds.headerProjectSelect).locator('option').count(),
    )
    .toBeGreaterThan(0)

  const caseKey = `SMK-${uniqueKey('').slice(-6)}`
  const caseTitle = `Smoke case ${caseKey}`
  await page.getByTestId(TestIds.caseListNewTitleInput).fill(caseTitle)
  await page.getByTestId(TestIds.caseListNewCaseButton).click()
  await expect(
    page.getByTestId(TestIds.caseListPanel).getByText(caseTitle),
  ).toBeVisible({
    timeout: 30_000,
  })

  await page.goto('/runs')
  await expect(page.getByTestId(TestIds.runsPage)).toBeVisible({
    timeout: 30_000,
  })
  const runName = `Smoke run ${caseKey}`
  await page.getByTestId(TestIds.runsCreateNameInput).fill(runName)
  await page.getByTestId(TestIds.runsCreateButton).click()
  await expect(page.getByText(runName).first()).toBeVisible({ timeout: 30_000 })

  await page.goto('/runs')
  await expect(page.getByTestId(TestIds.runsPage)).toBeVisible({
    timeout: 30_000,
  })
  await page.getByLabel('Upload target run').selectOption({ label: runName })
  await page
    .getByLabel('JUnit XML file')
    .setInputFiles(path.join(fixtures, 'sample-junit.xml'))
  await expect(page.getByText(/Ingested \d+ results/i)).toBeVisible({
    timeout: 30_000,
  })

  await page.getByTestId(TestIds.navLinkImport).click()
  await expect(page.getByTestId(TestIds.importPage)).toBeVisible()
  await page.getByLabel('Importer').selectOption({ label: 'TestLink' })
  await expect(page.getByLabel('Format')).toHaveValue('xml')
  const xml = fs.readFileSync(path.join(fixtures, 'testlink-sample.xml'), 'utf8')
  await page.getByLabel('Source content').fill(xml)
  await page.getByTestId(TestIds.importSubmitButton).click()
  await expect(page.getByText(/Created \d+ cases/i)).toBeVisible({
    timeout: 30_000,
  })

  await page.goto('/reports')
  await expect(page.getByTestId(TestIds.reportsPage)).toBeVisible({
    timeout: 30_000,
  })
  await expect(page.getByText('Recent run summaries')).toBeVisible()
  await expect(page.locator('.kpi-value').first()).toBeVisible()
})
