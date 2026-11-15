import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import {
  apiJson,
  createCase,
  createProject,
} from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { CasesPage } from '../pages/CasesPage'

test.describe.configure({ mode: 'serial' })

let projectKey = ''
let projectId = ''
let adminToken = ''
let caseKey = ''
let caseTitle = ''

test.beforeAll(async ({ request }) => {
  const admin = await ensureAdminSession(request)
  adminToken = admin.token
  projectKey = uniqueKey('AF')
  const project = await createProject(request, admin.token, {
    key: projectKey,
    name: `AutoFlag ${projectKey}`,
  })
  projectId = project.id
  caseTitle = `CI-only ${projectKey}`
  const created = await createCase(request, admin.token, projectId, {
    title: caseTitle,
    status: 'ready',
  })
  caseKey = created.case.key
})

async function selectProject(page: import('@playwright/test').Page) {
  const select = page.locator('.project-select')
  await expect.poll(async () => select.locator('option').count()).toBeGreaterThan(0)
  const options = await select.locator('option').allTextContents()
  const idx = options.findIndex((o) => o.includes(projectKey))
  if (idx >= 0) await select.selectOption({ index: idx })
}

test('CI results flip built-in Automated filter without external_key', {
  tag: ['@VQA-336', '@VUE-110'],
  annotation: [
    { type: 'vulyk', description: 'VQA-336' },
    { type: 'vulyk', description: 'VUE-110' },
  ],
}, async ({ page, request }) => {
  const opened = await apiJson<{
    run: { id: string }
  }>(request, 'POST', `/api/v1/projects/${projectId}/ci/runs`, {
    token: adminToken,
    data: {
      provider: 'github',
      repo: `acme/${projectKey}`,
      branch: 'main',
      commit: `c${uniqueKey('').slice(-6)}`,
      pipeline_url: `https://github.com/acme/${projectKey}/actions/runs/1`,
      suite_name: 'AutoFlag suite',
      external_ref: `e2e-af-${uniqueKey('')}`,
    },
  })
  const runId = opened.run.id

  const results = await apiJson<{
    inserted: number
    unmatched_case_keys: string[]
  }>(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs/${runId}/results`,
    {
      token: adminToken,
      data: {
        results: [
          {
            case_key: caseKey,
            status: 'passed',
            external_id: `spec::${projectKey}`,
            duration_ms: 8,
          },
        ],
      },
    },
  )
  expect(results.inserted).toBeGreaterThanOrEqual(1)
  expect(results.unmatched_case_keys).toEqual([])

  await loginAsAdmin(page)
  await selectProject(page)

  const cases = new CasesPage(page)
  await cases.open()
  await expect(cases.caseListHead()).toBeVisible({ timeout: 20_000 })

  await cases.openColumnsSettings()
  await cases.toggleColumn('automated', true)
  await expect(
    cases.caseListHead().getByTestId(TestIds.caseListColAutomated),
  ).toBeVisible()
  await cases.columnsButton().click()
  await expect(cases.columnsDialog()).toBeHidden()

  const row = page.locator(
    `[data-testid="${TestIds.caseRow}"][data-entity-key="${caseKey}"]`,
  )
  await expect(row).toBeVisible({ timeout: 15_000 })
  await expect(row.getByTestId(TestIds.caseRowAutomated)).toContainText(
    /Yes|Так|Oui|Sí|Ja/i,
  )

  await cases.filterAutomated('true')
  await expect(
    page.locator(
      `[data-testid="${TestIds.caseRow}"][data-entity-key="${caseKey}"]`,
    ),
  ).toBeVisible({ timeout: 15_000 })

  await cases.filterAutomated('false')
  await expect(
    page.locator(
      `[data-testid="${TestIds.caseRow}"][data-entity-key="${caseKey}"]`,
    ),
  ).toHaveCount(0)
})
