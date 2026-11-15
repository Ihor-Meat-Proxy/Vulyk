import { describe, expect, it } from 'vitest'
import {
  DEFAULT_LIVE_RUN_POLL_MS,
  hasLiveRuns,
  isLiveRunStatus,
  resolveLiveRunPollMs,
} from './liveRun'

describe('isLiveRunStatus', () => {
  it('treats in_progress and running as live', () => {
    expect(isLiveRunStatus('in_progress')).toBe(true)
    expect(isLiveRunStatus('In_Progress')).toBe(true)
    expect(isLiveRunStatus('running')).toBe(true)
    expect(isLiveRunStatus('in-progress')).toBe(true)
  })

  it('rejects terminal and idle statuses', () => {
    expect(isLiveRunStatus('closed')).toBe(false)
    expect(isLiveRunStatus('completed')).toBe(false)
    expect(isLiveRunStatus('cancelled')).toBe(false)
    expect(isLiveRunStatus('open')).toBe(false)
    expect(isLiveRunStatus('')).toBe(false)
    expect(isLiveRunStatus(null)).toBe(false)
  })
})

describe('hasLiveRuns', () => {
  it('is true when any run is live', () => {
    expect(
      hasLiveRuns([{ status: 'closed' }, { status: 'in_progress' }]),
    ).toBe(true)
    expect(hasLiveRuns([{ status: 'closed' }])).toBe(false)
    expect(hasLiveRuns([])).toBe(false)
  })
})

describe('resolveLiveRunPollMs', () => {
  it('defaults to 2s', () => {
    expect(resolveLiveRunPollMs({})).toBe(DEFAULT_LIVE_RUN_POLL_MS)
    expect(resolveLiveRunPollMs({ VITE_LIVE_RUN_POLL_MS: '' })).toBe(
      DEFAULT_LIVE_RUN_POLL_MS,
    )
    expect(resolveLiveRunPollMs({ VITE_LIVE_RUN_POLL_MS: 'nope' })).toBe(
      DEFAULT_LIVE_RUN_POLL_MS,
    )
  })

  it('honours a positive override', () => {
    expect(resolveLiveRunPollMs({ VITE_LIVE_RUN_POLL_MS: '500' })).toBe(500)
  })
})
