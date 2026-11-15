/**
 * Unit tests for Vulyk reporter helpers.
 * Run: node --experimental-strip-types --test reporters/vulyk-reporter-helpers.test.ts
 */
import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import {
  RESULT_BATCH_SIZE,
  closeCancelled,
  dedupePendingResults,
  dryRunApiResponse,
  formatDryRunLog,
  formatMissingAnnotationWarning,
  formatUnmatchedKeysWarning,
  isReporterDryRun,
  mapTestStatus,
  shouldFlushNow,
  takeFlushBatch,
  type PendingResult,
} from './vulyk-reporter-helpers.ts'

describe('mapTestStatus', () => {
  it('maps Playwright outcomes', () => {
    assert.equal(mapTestStatus('passed'), 'passed')
    assert.equal(mapTestStatus('failed'), 'failed')
    assert.equal(mapTestStatus('timedOut'), 'failed')
    assert.equal(mapTestStatus('interrupted'), 'failed')
    assert.equal(mapTestStatus('skipped'), 'skipped')
    assert.equal(mapTestStatus('unknown'), 'failed')
  })
})

describe('closeCancelled', () => {
  it('is true only for interrupted suite', () => {
    assert.equal(closeCancelled('interrupted'), true)
    assert.equal(closeCancelled('passed'), false)
    assert.equal(closeCancelled('failed'), false)
    assert.equal(closeCancelled('timedout'), false)
  })
})

describe('dedupePendingResults', () => {
  it('keeps last result per case_key', () => {
    const rows: PendingResult[] = [
      { case_key: 'VQA-1', status: 'failed', external_id: 'a' },
      { case_key: 'VQA-2', status: 'passed', external_id: 'b' },
      { case_key: 'VQA-1', status: 'passed', external_id: 'a-retry' },
    ]
    const out = dedupePendingResults(rows)
    assert.equal(out.length, 2)
    assert.equal(out.find((r) => r.case_key === 'VQA-1')?.status, 'passed')
    assert.equal(out.find((r) => r.case_key === 'VQA-1')?.external_id, 'a-retry')
  })
})

describe('takeFlushBatch / shouldFlushNow', () => {
  it('flushes when buffer reaches batch size', () => {
    assert.equal(shouldFlushNow(RESULT_BATCH_SIZE), true)
    assert.equal(shouldFlushNow(RESULT_BATCH_SIZE - 1), false)
  })

  it('splices a limited batch from the buffer', () => {
    const buf: PendingResult[] = [
      { case_key: 'A', status: 'passed', external_id: '1' },
      { case_key: 'B', status: 'passed', external_id: '2' },
      { case_key: 'C', status: 'passed', external_id: '3' },
    ]
    const batch = takeFlushBatch(buf, 2)
    assert.equal(batch.length, 2)
    assert.equal(buf.length, 1)
    assert.equal(buf[0].case_key, 'C')
  })
})

describe('isReporterDryRun / formatDryRunLog', () => {
  it('detects 1/true/yes and ignores other values', () => {
    assert.equal(isReporterDryRun({ VULYK_REPORTER_DRY_RUN: '1' }), true)
    assert.equal(isReporterDryRun({ VULYK_REPORTER_DRY_RUN: 'true' }), true)
    assert.equal(isReporterDryRun({ VULYK_REPORTER_DRY_RUN: 'YES' }), true)
    assert.equal(isReporterDryRun({ VULYK_REPORTER_DRY_RUN: '0' }), false)
    assert.equal(isReporterDryRun({}), false)
  })

  it('formats open/start/results/close log lines', () => {
    assert.equal(
      formatDryRunLog('open', 'POST /api/v1/projects/p/ci/runs'),
      '[vulyk-reporter] dry-run open: POST /api/v1/projects/p/ci/runs',
    )
    assert.match(formatDryRunLog('start', 'attach'), /dry-run start:/)
    assert.match(formatDryRunLog('results', 'n=2'), /dry-run results:/)
    assert.match(formatDryRunLog('close', 'cancelled=false'), /dry-run close:/)
  })

  it('dryRunApiResponse synthesizes open/results/close bodies', () => {
    const opened = dryRunApiResponse('POST', '/api/v1/projects/p/ci/runs', {
      start: true,
    }) as { run: { id: string; status: string } }
    assert.equal(opened.run.status, 'in_progress')
    assert.match(opened.run.id, /^dry-run-/)

    const results = dryRunApiResponse(
      'POST',
      '/api/v1/projects/p/ci/runs/r/results',
      { results: [{ case_key: 'VQA-1' }, { case_key: 'VQA-2' }] },
    ) as { inserted: number; unmatched_case_keys: string[] }
    assert.equal(results.inserted, 2)
    assert.deepEqual(results.unmatched_case_keys, [])

    const closed = dryRunApiResponse(
      'POST',
      '/api/v1/projects/p/ci/runs/r/close',
      { cancelled: false },
    ) as { status: string }
    assert.equal(closed.status, 'closed')
  })
})

describe('formatUnmatchedKeysWarning', () => {
  it('lists sample + count and warns once per key', () => {
    const warned = new Set<string>()
    const first = formatUnmatchedKeysWarning(
      [
        'VQA-9',
        'VQA-10',
        'VQA-9',
        'missing-1',
        'missing-2',
        'missing-3',
        'missing-4',
      ],
      warned,
      3,
    )
    assert.ok(first.message)
    assert.match(first.message!, /unmatched case keys: 6/)
    assert.match(first.message!, /sample: VQA-9, VQA-10, MISSING-1/)
    assert.match(first.message!, /\(\+3 more\)/)
    assert.match(first.message!, /type: 'vulyk'/)
    assert.equal(first.newlyWarned.length, 6)
    for (const k of first.newlyWarned) warned.add(k)

    const second = formatUnmatchedKeysWarning(['VQA-9', 'VQA-10'], warned, 3)
    assert.equal(second.message, null)
    assert.deepEqual(second.newlyWarned, [])

    const third = formatUnmatchedKeysWarning(['VQA-11'], warned, 3)
    assert.match(third.message!, /unmatched case keys: 1/)
    assert.match(third.message!, /sample: VQA-11/)
    assert.deepEqual(third.newlyWarned, ['VQA-11'])
  })
})

describe('formatMissingAnnotationWarning', () => {
  it('lists sample titles + count and points at annotation docs', () => {
    const msg = formatMissingAnnotationWarning(
      ['alpha', 'beta', 'gamma'],
      3,
      2,
    )
    assert.match(msg, /3 test\(s\) missing vulyk case key/)
    assert.match(msg, /sample: "alpha", "beta" \(\+1 more\)/)
    assert.match(msg, /tag @KEY-123/)
    assert.match(msg, /playwright-vulyk\.md/)
  })

  it('uses the project case-key prefix in the hint', () => {
    const msg = formatMissingAnnotationWarning(['alpha'], 1, 2, 'VUE')
    const hintKey = ['VUE', '123'].join('-')
    assert.ok(msg.includes(`description: '${hintKey}'`))
    assert.ok(msg.includes(`tag @${hintKey}`))
  })
})
