/**
 * Reporter DX helpers (dry-run + unmatched-key warnings).
 * These tests exercise pure helpers used by e2e/reporters/vulyk-reporter.ts;
 * they do not require a live Vulyk API.
 */
import { expect, test } from '@playwright/test'
import {
  dryRunApiResponse,
  formatDryRunLog,
  formatMissingAnnotationWarning,
  formatUnmatchedKeysWarning,
  isReporterDryRun,
} from '../reporters/vulyk-reporter-helpers'

test('dry-run helper logs open/start/results/close without POSTing', {
  tag: ['@VQA-363', '@VUE-225'],
  annotation: [
    { type: 'vulyk', description: 'VQA-363' },
    { type: 'vulyk', description: 'VUE-225' },
  ],
}, async () => {
  expect(isReporterDryRun({ VULYK_REPORTER_DRY_RUN: '1' })).toBe(true)
  expect(isReporterDryRun({ VULYK_REPORTER_DRY_RUN: '0' })).toBe(false)

  const openLog = formatDryRunLog(
    'open',
    'POST /api/v1/projects/p/ci/runs {"start":true}',
  )
  const startLog = formatDryRunLog('start', 'POST …/start')
  const resultsLog = formatDryRunLog('results', 'POST …/results n=2')
  const closeLog = formatDryRunLog('close', 'POST …/close')

  expect(openLog).toContain('[vulyk-reporter] dry-run open:')
  expect(startLog).toContain('dry-run start:')
  expect(resultsLog).toContain('dry-run results:')
  expect(closeLog).toContain('dry-run close:')

  const opened = dryRunApiResponse('POST', '/api/v1/projects/p/ci/runs', {
    start: true,
  }) as { run: { id: string; status: string } }
  expect(opened.run.status).toBe('in_progress')
  expect(opened.run.id).toMatch(/^dry-run-/)

  const posted = dryRunApiResponse(
    'POST',
    '/api/v1/projects/p/ci/runs/r/results',
    { results: [{ case_key: 'VQA-363' }] },
  ) as { inserted: number }
  expect(posted.inserted).toBe(1)
})

test('unmatched-key warning lists sample + count and warns once per key', {
  tag: ['@VQA-364', '@VUE-226'],
  annotation: [
    { type: 'vulyk', description: 'VQA-364' },
    { type: 'vulyk', description: 'VUE-226' },
  ],
}, async () => {
  const warned = new Set<string>()
  const first = formatUnmatchedKeysWarning(
    ['VQA-1', 'VQA-2', 'VQA-1', 'VQA-3', 'VQA-4', 'VQA-5', 'VQA-6'],
    warned,
    3,
  )
  expect(first.message).toBeTruthy()
  expect(first.message!).toMatch(/unmatched case keys: 6/)
  expect(first.message!).toMatch(/sample: VQA-1, VQA-2, VQA-3/)
  expect(first.message!).toMatch(/\(\+3 more\)/)
  expect(first.message!).toMatch(/type: 'vulyk'/)
  for (const k of first.newlyWarned) warned.add(k)

  const again = formatUnmatchedKeysWarning(['VQA-1', 'VQA-2'], warned, 3)
  expect(again.message).toBeNull()
  expect(again.newlyWarned).toEqual([])

  const missing = formatMissingAnnotationWarning(
    ['no key here', 'also missing'],
    2,
  )
  expect(missing).toMatch(/2 test\(s\) missing vulyk case key/)
  expect(missing).toMatch(/sample: "no key here", "also missing"/)
  expect(missing).toMatch(/playwright-vulyk\.md/)
})
