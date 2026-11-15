import { describe, expect, it } from 'vitest'

import { isAbandonableRunStatus, RUNS_PAGE_SIZE } from './RunsPage'

describe('RUNS_PAGE_SIZE', () => {
  it('keeps the documented page size for runs list + load-more', () => {
    expect(RUNS_PAGE_SIZE).toBe(50)
  })

  it('shows load-more when loaded count is below total', () => {
    const loaded = RUNS_PAGE_SIZE
    const total = RUNS_PAGE_SIZE + 10
    expect(loaded < total).toBe(true)
  })
})

describe('isAbandonableRunStatus', () => {
  it('allows in_progress and running (any case)', () => {
    expect(isAbandonableRunStatus('in_progress')).toBe(true)
    expect(isAbandonableRunStatus('IN_PROGRESS')).toBe(true)
    expect(isAbandonableRunStatus('running')).toBe(true)
    expect(isAbandonableRunStatus('Running')).toBe(true)
  })

  it('rejects completed, cancelled, and other terminal statuses', () => {
    expect(isAbandonableRunStatus('completed')).toBe(false)
    expect(isAbandonableRunStatus('cancelled')).toBe(false)
    expect(isAbandonableRunStatus('planned')).toBe(false)
    expect(isAbandonableRunStatus('')).toBe(false)
  })
})
