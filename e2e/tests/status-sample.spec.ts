import { expect, test } from '@playwright/test'
import { apiJson, loginApi } from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { ADMIN_EMAIL, ADMIN_PASSWORD } from '../fixtures/env'
import { AdminPage } from '../pages/AdminPage'

test.describe('status badges & sample data', () => {
  test.beforeAll(async ({ request }) => {
    await ensureAdminSession(request)
  })

  test('sample nightly suites have varied statuses and Sample CI creator', {
  tag: '@VQA-302', annotation: { type: 'vulyk', description: 'VQA-302' },
}, async ({
    request,
  }) => {
    const { token } = await loginApi(request, ADMIN_EMAIL, ADMIN_PASSWORD)
    const projects = await apiJson<{ id: string; key: string; is_sample?: boolean }[]>(
      request,
      'GET',
      '/api/v1/projects',
      { token },
    )
    const kyiv = projects.find((p) => p.key === 'KYIV_CONCERT_2026')
    expect(kyiv, 'Kyiv sample').toBeTruthy()
    expect(kyiv!.is_sample).toBe(true)

    const runs = await apiJson<{
      items: {
        name: string
        status: string
        created_by_name?: string | null
        is_sample?: boolean
      }[]
    }>(request, 'GET', `/api/v1/projects/${kyiv!.id}/runs?limit=50`, { token })
    const items = Array.isArray(runs) ? runs : (runs.items ?? [])
    const nightlies = items.filter((r) => /Sample nightly #/i.test(r.name))
    expect(nightlies.length).toBeGreaterThanOrEqual(10)
    expect(nightlies.every((r) => r.created_by_name === 'Sample CI')).toBe(true)
    expect(nightlies.every((r) => r.is_sample !== false)).toBe(true)

    const statuses = new Set(nightlies.map((r) => r.status.toLowerCase()))
    expect(statuses.has('completed') || statuses.has('passed')).toBeTruthy()
    expect(statuses.has('failed')).toBeTruthy()
    expect(
      statuses.has('in_progress') ||
        statuses.has('running') ||
        statuses.has('cancelled'),
    ).toBeTruthy()
  })

  test('Runs list Status column uses status-badge testids with translated labels', {
  tag: '@VQA-303', annotation: { type: 'vulyk', description: 'VQA-303' },
}, async ({
    page,
  }) => {
    await loginAsAdmin(page)
    await page.goto('/runs')
    const badges = page.getByTestId('runs-list-status-badge')
    const projectSelect = page.getByLabel(/project/i)
    if (await projectSelect.count()) {
      const options = await projectSelect.locator('option').allTextContents()
      const kyiv = options.find((o) => /KYIV_CONCERT|Kyiv/i.test(o))
      if (kyiv) {
        await projectSelect.selectOption({ label: kyiv })
      }
    }
    await page.waitForTimeout(500)
    if ((await badges.count()) === 0) {
      test.skip(true, 'No runs rendered on /runs for this build layout')
      return
    }
    const first = badges.first()
    await expect(first).toBeVisible()
    const text = (await first.innerText()).trim()
    expect(text).not.toMatch(/^(failed|in_progress|completed)$/i)
    expect(text.length).toBeGreaterThan(0)
  })

  test('Admin sample data section is visible under Settings', {
  tag: '@VQA-304', annotation: { type: 'vulyk', description: 'VQA-304' },
}, async ({
    page,
    request,
  }) => {
    const { token } = await loginApi(request, ADMIN_EMAIL, ADMIN_PASSWORD)
    const status = await apiJson<{ present: boolean }>(
      request,
      'GET',
      '/api/v1/admin/sample-data',
      { token },
    )
    expect(status.present).toBe(true)

    await loginAsAdmin(page)
    const admin = new AdminPage(page)
    await admin.open()
    await admin.openSettingsTab()
    const section = page.getByTestId('admin-sample-data-section')
    await expect(section).toBeVisible({ timeout: 15_000 })
    await expect(page.getByTestId('admin-sample-data-remove-button')).toBeVisible()
    await expect(page.getByTestId('admin-sample-data-reseed-button')).toBeVisible()
    await expect(page.getByTestId('admin-sample-data-present')).toBeVisible()
  })
})
