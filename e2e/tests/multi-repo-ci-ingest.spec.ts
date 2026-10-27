import { expect, test } from '@playwright/test'
import { TestIds } from '../../frontend/src/testing/testIds'
import {
  apiJson,
  createCase,
  createProject,
} from '../fixtures/api'
import { ensureAdminSession, loginAsAdmin } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'
import { SettingsPage } from '../pages/SettingsPage'

test.describe.configure({ mode: 'serial' })

let projectKey = ''
let projectId = ''
let adminToken = ''
let caseKey = ''

test.beforeAll(async ({ request }) => {
  const admin = await ensureAdminSession(request)
  adminToken = admin.token
  projectKey = uniqueKey('MR')
  const project = await createProject(request, admin.token, {
    key: projectKey,
    name: `MultiRepo ${projectKey}`,
  })
  projectId = project.id
  const created = await createCase(request, admin.token, projectId, {
    title: `CI case ${projectKey}`,
    status: 'ready',
  })
  caseKey = created.case.key
})

test('add two automation repos linked to one project', {
  tag: '@VQA-273', annotation: { type: 'vulyk', description: 'VQA-273' },
}, async ({ page }) => {
  await loginAsAdmin(page)
  const settings = new SettingsPage(page)
  await settings.open()
  await settings.expectAutomationHeading()

  const before = await settings.repoRows().count()
  const nameA = `repo-a-${projectKey}`
  const nameB = `repo-b-${projectKey}`

  await settings.fillNewRepo({
    name: nameA,
    remoteUrl: `https://github.com/example/${nameA}.git`,
    projectId,
  })
  await settings.submitAddRepo()
  await expect(settings.repoRows()).toHaveCount(before + 1, { timeout: 15_000 })

  await settings.fillNewRepo({
    name: nameB,
    remoteUrl: `https://github.com/example/${nameB}.git`,
    projectId,
  })
  await settings.submitAddRepo()
  await expect(settings.repoRows()).toHaveCount(before + 2, { timeout: 15_000 })
  await expect(
    page.locator(
      `[data-testid="${TestIds.settingsAutomationRepoRow}"]`,
    ),
  ).toHaveCount(before + 2)
})

test('create project CI token and reveal secret once', {
  tag: '@VQA-274', annotation: { type: 'vulyk', description: 'VQA-274' },
}, async ({ page }) => {
  await loginAsAdmin(page)
  const settings = new SettingsPage(page)
  await settings.open()
  await settings.expectCiTokensHeading()
  await settings.selectCiProject(projectId)
  await settings.createCiToken(`ci-${projectKey}`)
  await expect(settings.ciTokenSecret()).toContainText(/vulyk_ci_/, {
    timeout: 15_000,
  })
  await expect(settings.ciTokenRows().first()).toBeVisible()
})

test('CI open results close for two repo pipelines', {
  tag: '@VQA-275', annotation: { type: 'vulyk', description: 'VQA-275' },
}, async ({ request }) => {
  async function openRun(repo: string, externalRef: string) {
    return apiJson<{
      run: { id: string; status: string; lifecycle: string }
      idempotent: boolean
    }>(request, 'POST', `/api/v1/projects/${projectId}/ci/runs`, {
      token: adminToken,
      data: {
        provider: 'github',
        repo,
        branch: 'main',
        commit: `c${uniqueKey('').slice(-6)}`,
        pipeline_url: `https://github.com/${repo}/actions/runs/1`,
        suite_name: `Suite ${repo}`,
        external_ref: externalRef,
      },
    })
  }

  const refA = `e2e-a-${uniqueKey('')}`
  const refB = `e2e-b-${uniqueKey('')}`
  const openA = await openRun(`acme/${projectKey}-a`, refA)
  const openB = await openRun(`acme/${projectKey}-b`, refB)
  expect(openA.run.lifecycle).toBe('ci')
  expect(openB.run.lifecycle).toBe('ci')
  expect(openA.run.status).toBe('open')
  expect(openB.run.status).toBe('open')

  const resultsA = await apiJson<{
    inserted: number
    unmatched_case_keys: string[]
    status?: string
  }>(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs/${openA.run.id}/results`,
    {
      token: adminToken,
      data: {
        results: [
          {
            case_key: caseKey,
            status: 'passed',
            external_id: 'spec::a',
            duration_ms: 11,
          },
          {
            case_key: `${projectKey}-MISSING`,
            status: 'failed',
            external_id: 'spec::missing',
          },
        ],
      },
    },
  )
  expect(resultsA.inserted).toBe(1)
  expect(resultsA.unmatched_case_keys).toContain(`${projectKey}-MISSING`)

  await apiJson(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs/${openA.run.id}/close`,
    {
      token: adminToken,
      data: { message: 'repo-a done' },
    },
  )

  const resultsB = await apiJson<{ inserted: number }>(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs/${openB.run.id}/results`,
    {
      token: adminToken,
      data: {
        results: [
          {
            case_key: caseKey,
            status: 'failed',
            external_id: 'spec::b',
          },
        ],
      },
    },
  )
  expect(resultsB.inserted).toBe(1)

  await apiJson(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs/${openB.run.id}/close`,
    {
      token: adminToken,
      data: { message: 'repo-b done' },
    },
  )
})

test('JUnit ingest maps test_key property to case', {
  tag: '@VQA-276', annotation: { type: 'vulyk', description: 'VQA-276' },
}, async ({ request }) => {
  const opened = await apiJson<{ run: { id: string } }>(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs`,
    {
      token: adminToken,
      data: {
        provider: 'github',
        repo: `acme/${projectKey}-junit`,
        branch: 'main',
        commit: 'deadbeef',
        external_ref: `junit-${uniqueKey('')}`,
        suite_name: 'JUnit',
      },
    },
  )

  const junit = `<?xml version="1.0"?>
<testsuite name="s" tests="1">
  <testcase classname="t" name="login" time="0.05">
    <properties>
      <property name="test_key" value="${caseKey}"/>
    </properties>
  </testcase>
</testsuite>`

  const ingest = await apiJson<{ inserted: number }>(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs/${opened.run.id}/ingest`,
    {
      token: adminToken,
      data: { format: 'junit', content: junit, source: 'junit' },
    },
  )
  expect(ingest.inserted).toBeGreaterThanOrEqual(1)

  await apiJson(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs/${opened.run.id}/close`,
    {
      token: adminToken,
      data: {},
    },
  )
})
