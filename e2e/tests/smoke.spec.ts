import { expect, test } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
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

test('smoke: login → case → run → junit → import → report', async ({ page }) => {
  await loginAsAdmin(page)
  await expect(page.locator('.app-shell')).toBeVisible()
  await expect(page.getByRole('navigation', { name: 'Primary' })).toContainText(
    /Cases|Кейси/,
  )

  await expect(page.locator('.project-select')).toBeVisible()
  await expect
    .poll(async () => page.locator('.project-select option').count())
    .toBeGreaterThan(0)

  const caseKey = `SMK-${uniqueKey('').slice(-6)}`
  const caseTitle = `Smoke case ${caseKey}`
  await page.getByLabel('New case key').fill(caseKey)
  await page.getByLabel('New case title').fill(caseTitle)
  await page.getByRole('button', { name: 'Create case' }).click()
  await expect(page.getByText(caseTitle)).toBeVisible({ timeout: 30_000 })

  const nav = page.getByRole('navigation', { name: 'Primary' })
  await nav.getByRole('link', { name: /Runs|Прогони/i, exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Start a run' })).toBeVisible()
  const runName = `Smoke run ${caseKey}`
  await page.getByPlaceholder('Run name').fill(runName)
  await page.getByRole('button', { name: 'Create run' }).click()
  await expect(page.getByText(runName).first()).toBeVisible({ timeout: 30_000 })

  await nav.getByRole('link', { name: /Runs|Прогони/i, exact: true }).click()
  await expect(
    page.getByRole('heading', { name: 'Upload JUnit results' }),
  ).toBeVisible()
  await page.getByLabel('Upload target run').selectOption({ label: runName })
  await page
    .getByLabel('JUnit XML file')
    .setInputFiles(path.join(fixtures, 'sample-junit.xml'))
  await expect(page.getByText(/Ingested \d+ results/i)).toBeVisible({
    timeout: 30_000,
  })

  await nav.getByRole('link', { name: /Import|Імпорт/i }).click()
  await expect(page.getByRole('heading', { name: 'Import cases' })).toBeVisible()
  await page.getByLabel('Importer').selectOption({ label: 'TestLink' })
  await expect(page.getByLabel('Format')).toHaveValue('xml')
  const xml = fs.readFileSync(path.join(fixtures, 'testlink-sample.xml'), 'utf8')
  await page.getByLabel('Source content').fill(xml)
  await page.getByRole('button', { name: 'Import', exact: true }).click()
  await expect(page.getByText(/Created \d+ cases/i)).toBeVisible({
    timeout: 30_000,
  })

  await nav.getByRole('link', { name: /Reports|Звіти/i }).click()
  await expect(page.getByRole('heading', { name: 'Reports' })).toBeVisible()
  await expect(page.getByText('Recent run summaries')).toBeVisible()
  await expect(page.locator('.kpi-value').first()).toBeVisible()
})
