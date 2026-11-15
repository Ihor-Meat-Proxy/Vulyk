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
