import AxeBuilder from '@axe-core/playwright'
import { expect, test, type Page } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import {
  createCase,
  createProject,
  createSuite,
} from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { AdminPage } from '../pages/AdminPage'
import { GeneratePage } from '../pages/GeneratePage'
import { SettingsPage } from '../pages/SettingsPage'

test.describe.configure({ mode: 'serial' })

async function selectProject(page: Page, projectKey: string) {
  const select = page.locator('.project-select')
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })
}

test('Generate case picker exposes labels, pressed folders, and focusable chips', {
  tag: ['@VQA-368', '@VUE-230'],
  annotation: [
    { type: 'vulyk', description: 'VQA-368' },
    { type: 'vulyk', description: 'VUE-230' },
  ],
}, async ({ page, request }) => {
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('A11Y')
  const project = await createProject(request, admin.token, {
    key,
    name: `A11y picker ${key}`,
  })
  const suite = await createSuite(request, admin.token, project.id, 'Smoke')
  for (let i = 0; i < 4; i += 1) {
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

  const chips = page.getByTestId(TestIds.casePickerChips)
  await expect(chips).toHaveAttribute('aria-live', 'polite')
  await expect(chips).toHaveAttribute('role', 'group')

  const folders = page.getByTestId(TestIds.casePickerFolders)
  await expect(folders).toHaveAttribute('role', 'navigation')
  const allFolders = page.getByTestId(TestIds.casePickerAllFolders)
  await expect(allFolders).toHaveAttribute('aria-pressed', 'true')

  const folderBtn = folders.getByTestId(TestIds.casePickerFolder).first()
  await expect(folderBtn).toHaveAttribute(
    'aria-label',
    /Smoke|folder|папк|dossier|carpeta|Ordner/i,
  )
  await folderBtn.click()
  await expect(folderBtn).toHaveAttribute('aria-pressed', 'true')

  const selectFolder = folders
    .getByTestId(`${TestIds.casePickerFolder}-select`)
    .first()
  await expect(selectFolder).toHaveAttribute(
    'aria-label',
    /Select all cases in Smoke|Вибрати всі кейси|Sélectionner tous|Seleccionar todos|Alle Fälle in Smoke/i,
  )

  const checkbox = picker
    .getByTestId(TestIds.casePickerItem)
    .first()
    .locator('input[type="checkbox"]')
  await expect(checkbox).toHaveAttribute(
    'aria-label',
    /Select case|Вибрати кейс|Sélectionner le cas|Seleccionar caso|Fall .+ auswählen/i,
  )
  await checkbox.focus()
  await expect(checkbox).toBeFocused()
  await checkbox.check()

  const chip = chips.getByTestId(TestIds.casePickerChip).first()
  await expect(chip).toBeVisible()
  await expect(chip).toHaveAttribute(
    'aria-label',
    /Remove|Прибрати|Retirer|Quitar|entfernen/i,
  )
  await chip.focus()
  await expect(chip).toBeFocused()

  await page.getByTestId(TestIds.casePickerSearchInput).focus()
  await expect(page.getByTestId(TestIds.casePickerSearchInput)).toBeFocused()
  await page.keyboard.press('Tab')
  await expect(
    page
      .locator(
        `[data-testid="${TestIds.casePickerAllFolders}"], [data-testid="${TestIds.casePickerFolder}"]`,
      )
      .first(),
  ).toBeFocused()

  const axe = await new AxeBuilder({ page })
    .include(`[data-testid="${TestIds.casePicker}"]`)
    .withTags(['wcag2a', 'wcag2aa'])
    .disableRules(['color-contrast'])
    .analyze()
  const serious = axe.violations.filter(
    (v) => v.impact === 'critical' || v.impact === 'serious',
  )
  expect(
    serious,
    `axe serious/critical on case picker: ${JSON.stringify(
      serious.map((v) => ({ id: v.id, nodes: v.nodes.length })),
    )}`,
  ).toEqual([])
})

test('CI tokens copy controls and env block are labeled for assistive tech', {
  tag: ['@VQA-369', '@VUE-231'],
  annotation: [
    { type: 'vulyk', description: 'VQA-369' },
    { type: 'vulyk', description: 'VUE-231' },
  ],
}, async ({ page, request, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']).catch(() => {
    /* some browsers reject clipboard-read; write still works in Chromium */
  })
  const admin = await ensureAdminSession(request)
  const key = uniqueKey('A11YCI')
  const project = await createProject(request, admin.token, {
    key,
    name: `A11y CI ${key}`,
  })

  await loginAsAdmin(page)
  await selectProject(page, key)
  const settings = new SettingsPage(page)
  await settings.open()
  await settings.expectCiTokensHeading()
  await settings.selectCiProject(project.id)
  await settings.createCiToken(`ci-a11y-${key}`)

  const secret = settings.ciTokenSecret()
  await expect(secret).toBeVisible({ timeout: 15_000 })
  await expect(secret).toHaveAttribute('role', 'status')
  await expect(secret).toHaveAttribute('aria-live', 'polite')

  const copy = settings.ciTokenCopyButton()
  await expect(copy).toHaveAttribute(
    'aria-label',
    /Copy CI token|Скопіювати CI|Copier le jeton|Copiar token|CI-Token kopieren/i,
  )
  const copyEnv = page.getByTestId(TestIds.settingsCiTokenCopyEnvButton)
  await expect(copyEnv).toHaveAttribute(
    'aria-label',
    /environment|середовища|environnement|entorno|Umgebung/i,
  )

  const env = settings.ciTokenEnvBlock()
  // Focusable readonly snippet (not a landmark): textbox + aria-readonly.
  await expect(env).toHaveAttribute('role', 'textbox')
  await expect(env).toHaveAttribute('aria-readonly', 'true')
  await expect(env).toHaveAttribute(
    'aria-label',
    /environment|середовища|environnement|entorno|Umgebung/i,
  )
  await env.focus()
  await expect(env).toBeFocused()

  await copy.click()
  const notice = page.getByTestId(TestIds.settingsCiTokenCopyNotice)
  await expect(notice).toBeVisible({ timeout: 10_000 })
  await expect(notice).toHaveAttribute('role', 'status')
  await expect(notice).toHaveAttribute('aria-live', 'polite')

  const axe = await new AxeBuilder({ page })
    .include(`[data-testid="${TestIds.settingsCiPanel}"]`)
    .withTags(['wcag2a', 'wcag2aa'])
    .disableRules(['color-contrast'])
    .analyze()
  const serious = axe.violations.filter(
    (v) => v.impact === 'critical' || v.impact === 'serious',
  )
  expect(
    serious,
    `axe serious/critical on CI tokens: ${JSON.stringify(
      serious.map((v) => ({ id: v.id, nodes: v.nodes.length })),
    )}`,
  ).toEqual([])
})

test('Updates check error region uses alert live region', {
  tag: ['@VQA-370', '@VUE-232'],
  annotation: [
    { type: 'vulyk', description: 'VQA-370' },
    { type: 'vulyk', description: 'VUE-232' },
  ],
}, async ({ page, request }) => {
  await ensureAdminSession(request)
  await loginAsAdmin(page)
  const admin = new AdminPage(page)
  await admin.open()
  await admin.openTab('updates')
  await expect(page.getByTestId(TestIds.adminUpdatesPanel)).toBeVisible({
    timeout: 20_000,
  })

  // Match ?force=true (Check/Retry bypasses backoff via query string).
  await page.route('**/api/v1/admin/updates/check*', async (route) => {
    if (route.request().method() !== 'POST') {
      await route.continue()
      return
    }
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        current_version: '0.0.0-test',
        latest_version: null,
        latest_notes: null,
        latest_url: null,
        update_available: false,
        last_checked_at: null,
        last_check_error_code: 'git_host_rate_limit',
        last_check_error_at: '2024-06-01T12:00:00Z',
        last_check_error_reset_at: null,
        check_interval_hours: 24,
        capability: 'manual_only',
        manual_command: './scripts/install.sh upgrade',
        rollback_command: './scripts/install.sh rollback',
        github_repo: 'Ihor-Meat-Proxy/Vulyk',
        active_job: null,
        rollback: null,
        version_backups: [],
        retention: { rollback_days: 7, version_count: 3 },
      }),
    })
  })

  await page.getByTestId(TestIds.adminUpdatesCheckButton).click()

  const checkError = page.getByTestId(TestIds.adminUpdatesCheckError)
  await expect(checkError).toBeVisible({ timeout: 15_000 })
  await expect(checkError).toHaveAttribute('role', 'alert')
  await expect(checkError).toHaveAttribute('aria-live', 'assertive')
  await expect(checkError).toHaveAttribute(
    'aria-label',
    /Update check error|Помилка перевірки|Erreur de vérification|Error de comprobación|Fehler bei der Update/i,
  )

  const status = page.getByTestId(TestIds.adminUpdatesStatus)
  await expect(status).toHaveAttribute('role', 'alert')
  await expect(status).toHaveAttribute('aria-live', 'assertive')
})

test('In-progress status badge exposes text label not colour alone', {
  tag: ['@VQA-371', '@VUE-233'],
  annotation: [
    { type: 'vulyk', description: 'VQA-371' },
    { type: 'vulyk', description: 'VUE-233' },
  ],
}, async ({ page, request }) => {
  await ensureAdminSession(request)
  await loginAsAdmin(page)
  await page.goto('/appearance')
  await expect(page.locator('.appearance-preview')).toBeVisible({
    timeout: 30_000,
  })

  const pill = page
    .locator(
      '.appearance-preview .status-pill[data-status="In_progress"], .appearance-preview .status-pill[data-status="Running"]',
    )
    .first()
  await expect(pill).toBeVisible()
  await expect(pill.locator('.status-label')).not.toBeEmpty()
  await expect(pill.locator('.status-icon')).toBeVisible()
  await expect(pill).toHaveAttribute('role', 'status')
  await expect(pill).toHaveAttribute('aria-live', 'polite')
  await expect(pill).toHaveAttribute('aria-label', /.+/)
})
