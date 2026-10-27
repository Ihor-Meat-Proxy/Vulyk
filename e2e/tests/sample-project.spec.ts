import { expect, test } from '@playwright/test'
import { apiJson, loginApi } from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { ADMIN_EMAIL, ADMIN_PASSWORD } from '../fixtures/env'

test.describe('sample project seed', () => {
  test.beforeAll(async ({ request }) => {
    await ensureAdminSession(request)
  })

  test('Kyiv concert sample project, suites, cases, and two runs exist', {
  tag: '@VQA-297', annotation: { type: 'vulyk', description: 'VQA-297' },
}, async ({
    request,
  }) => {
    const { token } = await loginApi(request, ADMIN_EMAIL, ADMIN_PASSWORD)
    const projects = await apiJson<{ id: string; key: string; name: string }[]>(
      request,
      'GET',
      '/api/v1/projects',
      { token },
    )
    const sample = projects.find((p) => p.key === 'KYIV_CONCERT_2026')
    expect(sample, 'sample project KYIV_CONCERT_2026').toBeTruthy()
    expect(sample!.name).toMatch(/Kyiv|Київ/i)

    const suites = await apiJson<{ id: string; name: string }[]>(
      request,
      'GET',
      `/api/v1/projects/${sample!.id}/suites`,
      { token },
    )
    expect(suites.length).toBeGreaterThanOrEqual(9)
    const names = suites.map((s) => s.name).join(' ')
    expect(names).toMatch(/Venue|Ticketing|Security|Marketing/i)

    const cases = await apiJson<{ items: unknown[]; total?: number }>(
      request,
      'GET',
      `/api/v1/projects/${sample!.id}/cases?limit=100`,
      { token },
    )
    const caseItems = Array.isArray(cases) ? cases : (cases.items ?? [])
    expect(caseItems.length).toBeGreaterThanOrEqual(25)
    expect(caseItems.length).toBeLessThanOrEqual(50)

    const runs = await apiJson<{
      items: { name: string; status: string }[]
    }>(request, 'GET', `/api/v1/projects/${sample!.id}/runs?limit=20`, {
      token,
    })
    const runItems = Array.isArray(runs) ? runs : (runs.items ?? [])
    expect(runItems.length).toBeGreaterThanOrEqual(2)
    const runNames = runItems.map((r) => r.name).join(' | ')
    expect(runNames).toMatch(/Dress rehearsal/i)
    expect(runNames).toMatch(/Ticketing smoke/i)

    const dress = runItems.find((r) => /Dress rehearsal/i.test(r.name))
    const smoke = runItems.find((r) => /Ticketing smoke/i.test(r.name))
    expect(dress?.status).toMatch(/completed|closed/i)
    expect(smoke?.status).toMatch(/in_progress|running/i)
  })

  test('Vulyk QA sample project and area folders appear', {
  tag: '@VQA-298', annotation: { type: 'vulyk', description: 'VQA-298' },
}, async ({
    request,
    page,
  }) => {
    const { token } = await loginApi(request, ADMIN_EMAIL, ADMIN_PASSWORD)
    const projects = await apiJson<{ id: string; key: string; name: string }[]>(
      request,
      'GET',
      '/api/v1/projects',
      { token },
    )
    const sample = projects.find((p) => p.key === 'VULYK_QA')
    expect(sample, 'sample project VULYK_QA').toBeTruthy()
    expect(sample!.name).toMatch(/Vulyk QA/i)

    const suites = await apiJson<{ id: string; name: string }[]>(
      request,
      'GET',
      `/api/v1/projects/${sample!.id}/suites`,
      { token },
    )
    expect(suites.length).toBeGreaterThanOrEqual(20)
    const names = suites.map((s) => s.name).join(' | ')
    expect(names).toMatch(/Setup|Install|Встановлення/i)
    expect(names).toMatch(/Authentication|Автентифікація/i)
    expect(names).toMatch(/RBAC/i)
    expect(names).toMatch(/Folders|Теки/i)
    expect(names).toMatch(/Import|Імпорт/i)
    expect(names).toMatch(/LLM/i)
    expect(names).toMatch(/MCP/i)
    expect(names).toMatch(/Appearance|Вигляд/i)
    expect(names).toMatch(/Header|Шапка|settings|налаштування/i)
    expect(names).toMatch(/PWA/i)
    expect(names).toMatch(/Performance|Продуктивність/i)

    const cases = await apiJson<{
      items: { key: string }[]
      total?: number
    }>(request, 'GET', `/api/v1/projects/${sample!.id}/cases?limit=5`, {
      token,
    })
    const total =
      !Array.isArray(cases) && typeof cases.total === 'number'
        ? cases.total
        : (Array.isArray(cases) ? cases : cases.items ?? []).length
    expect(total).toBeGreaterThanOrEqual(120)
    // Upper bound grows as feature fragments add Vulyk QA cases
    // (columns, pdf, lists/CF, coverage hardening, playwright-e2e 1:1, …).
    expect(total).toBeLessThanOrEqual(400)

    const runs = await apiJson<{
      items: { name: string; status: string }[]
    }>(request, 'GET', `/api/v1/projects/${sample!.id}/runs?limit=20`, {
      token,
    })
    const runItems = Array.isArray(runs) ? runs : (runs.items ?? [])
    expect(runItems.length).toBeGreaterThanOrEqual(2)
    expect(runItems.map((r) => r.name).join(' | ')).toMatch(/Vulyk QA smoke/i)

    await loginAsAdmin(page)
    const select = page.locator('.project-select')
    await expect
      .poll(async () => select.locator('option').count())
      .toBeGreaterThan(0)
    const options = await select.locator('option').allTextContents()
    const idx = options.findIndex((o) => /Vulyk QA/i.test(o))
    expect(idx, 'Vulyk QA in project switcher').toBeGreaterThanOrEqual(0)
    await select.selectOption({ index: idx })
    // Sole folder UI is the Cases SquashTM tree — not a sidebar Folders section.
    await expect(page.locator('.sidebar .suite-block')).toHaveCount(0)
    await expect(
      page.getByRole('region', {
        name: /Case folders|Теки кейсів|Dossiers de cas|Carpetas de casos|Fallordner/i,
      }),
    ).toBeVisible({ timeout: 30_000 })
    await expect(
      page.locator('.folder-tree').getByTestId('folder-all-cases'),
    ).toBeVisible({ timeout: 20_000 })
    await expect(
      page
        .locator('.folder-tree')
        .getByRole('button', { name: /01\. Setup|01\. Встановлення/i })
        .first(),
    ).toBeVisible({ timeout: 30_000 })
  })
})
