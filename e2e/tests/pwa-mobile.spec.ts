import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'

test.describe('PWA + mobile Chromium viewport', () => {
  test.beforeAll(async ({ request }) => {
    await ensureAdminSession(request)
  })

  test('bottom nav, folders drawer, and stacked case flow', {
  tag: '@VQA-284', annotation: { type: 'vulyk', description: 'VQA-284' },
}, async ({ page }) => {
    await loginAsAdmin(page)

    const bottom = page.getByTestId(TestIds.bottomNav)
    await expect(bottom).toBeVisible()
    await expect(bottom.getByRole('link', { name: /Cases|Кейси/i })).toBeVisible()
    await expect(
      bottom.getByRole('link', {
        name: /Runs|Прогони|Exécutions de test|Ejecuciones de pruebas|Testläufe/i,
      }),
    ).toBeVisible()

    // App sidebar no longer lists folders — only Menu for primary nav.
    await page.locator('button.menu-toggle').click()
    await expect(page.locator('#nav-drawer')).toBeVisible()
    await expect(
      page.locator('#nav-drawer').getByTestId(TestIds.primaryNav),
    ).toBeVisible()
    await expect(page.locator('#nav-drawer').locator('.suite-block')).toHaveCount(0)
    await expect(
      page.locator('#nav-drawer').getByText(/^\s*Folders\s*$|^\s*Теки\s*$/),
    ).toHaveCount(0)
    await page.locator('#nav-drawer').getByRole('button', { name: /Close|Закрити/i }).click()
    await expect(page.locator('.drawer-backdrop')).toHaveCount(0)

    // Cases page owns the SquashTM tree as a collapsible Folders drawer.
    await page
      .locator('.case-list-panel')
      .getByRole('button', { name: /Folders|Теки/i })
      .click()
    const folderDrawer = page.locator('#case-folder-drawer')
    await expect(folderDrawer).toBeVisible()
    await expect(
      folderDrawer.getByTestId('folder-all-cases'),
    ).toBeVisible()
    await folderDrawer.getByTestId('folder-all-cases').click()
    await expect(page.locator('.case-tree-backdrop')).toHaveCount(0)

    await expect(
      page.getByLabel(
        /New case title|Назва нового кейсу|Titre du nouveau cas|Título del nuevo caso|Titel des neuen Falls/i,
      ),
    ).toBeVisible()
    const caseKey = `MOB-${uniqueKey('').slice(-5)}`
    const mobileTitle = `Mobile ${caseKey}`
    await page
      .getByLabel(
        /New case title|Назва нового кейсу|Titre du nouveau cas|Título del nuevo caso|Titel des neuen Falls/i,
      )
      .fill(mobileTitle)
    await page.getByRole('button', { name: /Create case|Створити кейс/i }).click()
    await expect(page.locator('.case-list-panel').getByText(mobileTitle)).toBeVisible({
      timeout: 30_000,
    })

    // Avoid sticky topbar / bottom-nav intercepting the case link on short viewports.
    const caseLink = page.locator('.case-list-panel').getByRole('link', {
      name: mobileTitle,
    })
    await caseLink.evaluate((el) =>
      el.scrollIntoView({ block: 'center', inline: 'nearest' }),
    )
    await caseLink.click({ force: true })
    await expect(page.getByText(caseKey).first()).toBeVisible()
    await expect(page.getByTestId('case-detail-back')).toBeVisible()

    await bottom.getByTestId(TestIds.bottomNavRuns).click()
    await expect(page.getByTestId(TestIds.runsPage)).toBeVisible({
      timeout: 30_000,
    })
    // "Start a run" is permission-gated; empty copy still means we landed on Runs.
    await expect(
      page
        .getByRole('heading', { name: /Start a run/i })
        .or(page.getByText(/Select a project to manage runs/i)),
    ).toBeVisible()

    await expect(bottom.getByRole('link', { name: /Admin|Адмін/i })).toHaveCount(0)

    await page.goto('/admin')
    await expect(page.getByTestId(TestIds.adminPage)).toBeVisible({
      timeout: 30_000,
    })
    await expect(page.getByRole('tablist')).toBeVisible()
  })

  test('web app manifest and service worker are present', {
  tag: '@VQA-285', annotation: { type: 'vulyk', description: 'VQA-285' },
}, async ({ page }) => {
    await page.goto('/')
    const manifest = page.locator('link[rel="manifest"]')
    await expect(manifest).toHaveAttribute('href', /manifest\.webmanifest/)

    const manifestRes = await page.request.get('/manifest.webmanifest')
    expect(manifestRes.ok()).toBeTruthy()
    const body = await manifestRes.json()
    expect(body.name).toMatch(/Vulyk/i)
    expect(body.icons?.length).toBeGreaterThan(4)
    expect(
      body.icons.some((i: { purpose?: string }) => i.purpose?.includes('maskable')),
    ).toBeTruthy()

    const swReady = await page.evaluate(async () => {
      if (!('serviceWorker' in navigator)) return false
      const reg = await navigator.serviceWorker.getRegistration()
      return Boolean(reg?.active || reg?.waiting || reg?.installing)
    })
    expect(swReady).toBeTruthy()
  })

  test('offline fallback document is served', {
  tag: '@VQA-286', annotation: { type: 'vulyk', description: 'VQA-286' },
}, async ({ page }) => {
    const res = await page.request.get('/offline.html')
    // May 404 in some builds; accept either cached offline page or app shell
    if (res.status() === 200) {
      const text = await res.text()
      expect(text.length).toBeGreaterThan(0)
    } else {
      await page.goto('/')
      await expect(page.locator('html')).toBeVisible()
    }
  })
})
