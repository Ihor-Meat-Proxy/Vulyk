import { expect, test, type Page } from '@playwright/test'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'

async function openCaseInRun(page: Page, caseKey: string) {
  const back = page.getByRole('button', { name: '← Queue' })
  if (await back.isVisible().catch(() => false)) {
    await back.click()
  }
  // Prefer the accessible run-case list; fall back to the desktop table row.
  const listBtn = page
    .getByRole('list', { name: 'Run cases' })
    .getByRole('button', { name: new RegExp(caseKey) })
    .first()
  const tableRow = page.locator('tr', { hasText: caseKey }).first()
  if (await listBtn.isVisible().catch(() => false)) {
    await listBtn.click()
  } else {
    await expect(tableRow).toBeVisible({ timeout: 15_000 })
    await tableRow.click()
  }

  const statusGroup = page.getByRole('group', { name: 'Result status' })
  await expect(statusGroup).toBeVisible({ timeout: 20_000 })
  await expect(
    statusGroup.getByRole('button', { name: 'Passed', exact: true }),
  ).toBeEnabled({ timeout: 15_000 })
}

function statusButton(page: Page, label: string) {
  return page
    .getByRole('group', { name: 'Result status' })
    .getByRole('button', { name: label, exact: true })
}

test.describe.configure({ mode: 'serial' })

test.beforeAll(async ({ request }) => {
  await ensureAdminSession(request)
})

test('mark Passed→Failed→Blocked→Skipped; timeline and case History tab', async ({
  page,
}) => {
  await loginAsAdmin(page)

  const caseKey = `HIST-${uniqueKey('').slice(-6)}`
  const caseTitle = `History case ${caseKey}`
  await page.getByLabel(/New case title|Назва нового кейсу/i).fill(caseTitle)
  await page.getByRole('button', { name: /Create case|Створити кейс/i }).click()
  await expect(page.getByText(caseTitle)).toBeVisible({ timeout: 30_000 })

  await page.getByRole('link', { name: caseTitle }).click()
  await expect(page.getByRole('heading', { level: 2 })).toContainText(caseTitle)
  await page.getByRole('button', { name: 'Edit' }).click()
  await page.getByRole('button', { name: 'Add step' }).click()
  await page.locator('.step textarea').first().fill('Open app')
  await page.locator('.step textarea').nth(1).fill('App loads')
  await page.getByRole('button', { name: 'Save' }).click()
  await expect(page.getByText('Open app')).toBeVisible()

  const nav = page.getByRole('navigation', { name: 'Primary' })
  await nav.getByRole('link', { name: /Runs|Прогони/i, exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Start a run' })).toBeVisible()
  const runName = `History run ${caseKey}`
  await page.getByPlaceholder('Run name').fill(runName)
  await page.getByRole('button', { name: 'Create run' }).click()
  await expect(page.getByRole('heading', { name: runName })).toBeVisible({
    timeout: 30_000,
  })

  const statuses = ['Passed', 'Failed', 'Blocked', 'Skipped'] as const
  for (const status of statuses) {
    await openCaseInRun(page, caseKey)
    const btn = statusButton(page, status)
    await expect(btn.first()).toBeEnabled()
    await btn.first().click()

    // After save the UI advances/closes the exec panel; assert on the queue pill.
    await expect(
      page
        .locator(`.status-pill.status-${status.toLowerCase()}`)
        .filter({ hasText: status })
        .first(),
    ).toBeVisible({ timeout: 20_000 })
  }

  // Optional richer history UI (when present)
  const historyTab = page.getByRole('tab', { name: 'History / Results' })
  if (await historyTab.isVisible().catch(() => false)) {
    await nav.getByRole('link', { name: /Cases|Кейси/i }).click()
    await page.getByRole('link', { name: caseTitle }).click()
    await historyTab.click()
    const caseTimeline = page.getByLabel('Case results across runs')
    await expect(caseTimeline).toBeVisible({ timeout: 30_000 })
  }
})
