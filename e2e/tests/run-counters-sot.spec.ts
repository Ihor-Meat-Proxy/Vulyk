import { expect, test } from '@playwright/test'
import {
  apiJson,
  createCase,
  createProject,
} from '../fixtures/api'
import { ensureAdminSession } from '../fixtures/auth'
import { uniqueKey } from '../fixtures/env'

/**
 * Run counters SoT: after CI ingest, Run detail progress, denormalized
 * test_runs.*_count, and Reports summary must agree (one status per case).
 */
test.describe.configure({ mode: 'serial' })

let projectKey = ''
let projectId = ''
let adminToken = ''
let caseKeyA = ''
let caseKeyB = ''

test.beforeAll(async ({ request }) => {
  const admin = await ensureAdminSession(request)
  adminToken = admin.token
  projectKey = uniqueKey('SOT')
  const project = await createProject(request, admin.token, {
    key: projectKey,
    name: `Counters SoT ${projectKey}`,
  })
  projectId = project.id
  const a = await createCase(request, admin.token, projectId, {
    title: `SoT A ${projectKey}`,
    status: 'ready',
  })
  caseKeyA = a.case.key
  const b = await createCase(request, admin.token, projectId, {
    title: `SoT B ${projectKey}`,
    status: 'ready',
  })
  caseKeyB = b.case.key
})

test('CI run detail progress matches Reports summary counters', {
  tag: ['@VQA-355', '@VUE-217'],
  annotation: [
    { type: 'vulyk', description: 'VQA-355' },
    { type: 'vulyk', description: 'VUE-217' },
  ],
}, async ({ request }) => {
  const opened = await apiJson<{ run: { id: string } }>(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs`,
    {
      token: adminToken,
      data: {
        provider: 'github',
        repo: `acme/${projectKey}-sot`,
        branch: 'main',
        commit: 'sot001',
        external_ref: `sot-${uniqueKey('')}`,
        suite_name: `SoT ${projectKey}`,
      },
    },
  )

  // Two attempts for case A + one skip for B → 2 cases in progress, not 3 result rows.
  await apiJson(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs/${opened.run.id}/results`,
    {
      token: adminToken,
      data: {
        results: [
          {
            case_key: caseKeyA,
            status: 'failed',
            external_id: 'sot-a-fail',
            duration_ms: 5,
          },
          {
            case_key: caseKeyA,
            status: 'passed',
            external_id: 'sot-a-pass',
            duration_ms: 7,
          },
          {
            case_key: caseKeyB,
            status: 'skipped',
            external_id: 'sot-b',
            duration_ms: 1,
          },
        ],
      },
    },
  )

  await apiJson(
    request,
    'POST',
    `/api/v1/projects/${projectId}/ci/runs/${opened.run.id}/close`,
    { token: adminToken, data: { message: 'sot done' } },
  )

  const detail = await apiJson<{
    run: {
      pass_count: number
      fail_count: number
      skip_count: number
      blocked_count: number
    }
    progress: {
      total: number
      passed: number
      failed: number
      skipped: number
      blocked: number
    }
  }>(request, 'GET', `/api/v1/projects/${projectId}/runs/${opened.run.id}`, {
    token: adminToken,
  })

  const summary = await apiJson<{
    counts: {
      total: number
      passed: number
      failed: number
      skipped: number
      blocked: number
    }
  }>(
    request,
    'GET',
    `/api/v1/projects/${projectId}/reports/runs/${opened.run.id}/summary`,
    { token: adminToken },
  )

  expect(detail.progress.total).toBe(2)
  expect(detail.progress.passed).toBe(1)
  expect(detail.progress.skipped).toBe(1)
  expect(detail.progress.failed).toBe(0)

  expect(summary.counts.total).toBe(detail.progress.total)
  expect(summary.counts.passed).toBe(detail.progress.passed)
  expect(summary.counts.failed).toBe(detail.progress.failed)
  expect(summary.counts.skipped).toBe(detail.progress.skipped)
  expect(summary.counts.blocked).toBe(detail.progress.blocked)

  expect(detail.run.pass_count).toBe(detail.progress.passed)
  expect(detail.run.fail_count).toBe(detail.progress.failed)
  expect(detail.run.skip_count).toBe(detail.progress.skipped)
  expect(detail.run.blocked_count).toBe(detail.progress.blocked)
})
