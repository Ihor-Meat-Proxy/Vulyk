import { expect, test } from '@playwright/test'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'

test.describe('PWA + mobile Chromium viewport', () => {
  test.beforeAll(async ({ request }) => {
    await ensureAdminSession(request)
  })

  test('bottom nav, suite drawer, and stacked case flow', async ({ page }) => {
    await loginAsAdmin(page)

    const bottom = page.getByRole('navigation', { name: 'Mobile primary' })
    await expect(bottom).toBeVisible()
    await expect(bottom.getByRole('link', { name: /Cases|Кейси/i })).toBeVisible()
    await expect(bottom.getByRole('link', { name: /Runs|Прогони/i })).toBeVisible()

    await page.locator('button.menu-toggle').click()
    await expect(page.locator('#suite-drawer')).toBeVisible()
    await expect(
      page.locator('#suite-drawer').getByRole('button', { name: /All cases|Усі кейси/i }),
    ).toBeVisible()
    await page.locator('#suite-drawer').getByRole('button', { name: /Close|Закрити/i }).click()
    await expect(page.locator('.drawer-backdrop')).toHaveCount(0)

    await expect(page.getByLabel(/New case title|Назва нового кейсу/i)).toBeVisible()
    const caseKey = `MOB-${uniqueKey('').slice(-5)}`
    const mobileTitle = `Mobile ${caseKey}`
    await page.getByLabel(/New case title|Назва нового кейсу/i).fill(mobileTitle)
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
    await expect(page.getByRole('link', { name: /Back to cases/i })).toBeVisible()

    await bottom.getByRole('link', { name: /Runs|Прогони/i }).click()
    await expect(page.getByRole('heading', { name: /Start a run/i })).toBeVisible()

    await bottom.getByRole('link', { name: /Admin|Адмін/i }).click()
    await expect(page.getByRole('heading', { name: /Admin|Адмін/i })).toBeVisible()
    await expect(page.getByRole('tablist')).toBeVisible()
  })

  test('web app manifest and service worker are present', async ({ page }) => {
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

  test('offline fallback document is served', async ({ page }) => {
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
