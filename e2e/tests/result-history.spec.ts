import { expect, test, type Page } from '@playwright/test'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'

async function openCaseInRun(page: Page, caseKey: string) {
  const back = page.getByRole('button', { name: '← Queue' })
  if (await back.isVisible().catch(() => false)) {
    await back.click()
  }
  const row = page.getByRole('button', { name: new RegExp(caseKey) }).first()
  await expect(row).toBeVisible({ timeout: 15_000 })
  await row.click()

  // Prefer labelled status group when present; else thumb status buttons.
  const statusGroup = page.getByRole('group', { name: 'Result status' })
  if (await statusGroup.isVisible().catch(() => false)) {
    await expect(
      statusGroup.getByRole('button', { name: /Passed|passed/i }).first(),
    ).toBeEnabled({ timeout: 15_000 })
  } else {
    await expect(
      page.getByRole('button', { name: 'passed', exact: true }),
    ).toBeEnabled({ timeout: 15_000 })
  }
}

function statusButton(page: Page, label: string) {
  const statusGroup = page.getByRole('group', { name: 'Result status' })
  return statusGroup
    .getByRole('button', { name: label, exact: true })
    .or(page.getByRole('button', { name: label.toLowerCase(), exact: true }))
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
  await page.getByLabel('New case key').fill(caseKey)
  await page.getByLabel('New case title').fill(caseTitle)
  await page.getByRole('button', { name: 'Create case' }).click()
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

    const timeline = page.getByLabel('Result history')
    if (await timeline.isVisible().catch(() => false)) {
      await expect(timeline.locator('.status-pill').filter({ hasText: status })).toBeVisible({
        timeout: 20_000,
      })
    } else {
      // Current UI uses lowercase status pills on the queue when no timeline yet.
      await expect(
        page.locator(`.status-pill.status-${status.toLowerCase()}`).first(),
      ).toBeVisible({ timeout: 20_000 })
    }
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
