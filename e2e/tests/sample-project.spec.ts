import { expect, test } from '@playwright/test'
import { apiJson, loginApi } from '../fixtures/api'
import { ADMIN_EMAIL, ADMIN_PASSWORD } from '../fixtures/env'

test.describe('sample project seed', () => {
  test('Kyiv concert sample project, suites, cases, and two runs exist', async ({
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
})
