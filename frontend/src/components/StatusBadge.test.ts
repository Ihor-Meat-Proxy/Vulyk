import { describe, expect, it } from 'vitest'
import { statusClass } from '../api'
import { statusLabelKey } from './StatusBadge'
import { statusVisualKind } from './StatusIcon'

describe('StatusBadge shared status mapping', () => {
  it('maps result statuses to i18n keys', () => {
    expect(statusLabelKey('Passed')).toBe('status.passed')
    expect(statusLabelKey('failed')).toBe('status.failed')
    expect(statusLabelKey('IN_PROGRESS')).toBe('status.inProgress')
    expect(statusLabelKey('in-progress')).toBe('status.inProgress')
    expect(statusLabelKey('completed')).toBe('status.completed')
    expect(statusLabelKey('cancelled')).toBe('status.cancelled')
    expect(statusLabelKey('open')).toBe('status.open')
    expect(statusLabelKey('closed')).toBe('status.closed')
    expect(statusLabelKey('queued')).toBe('status.queued')
    expect(statusLabelKey('succeeded')).toBe('status.succeeded')
  })

  it('never falls back to empty key for known run/CI statuses', () => {
    for (const s of [
      'failed',
      'in_progress',
      'completed',
      'cancelled',
      'blocked',
      'skipped',
      'untestable',
      'open',
      'closed',
      'draft',
      'dispatched',
    ]) {
      expect(statusLabelKey(s)).not.toBe('')
    }
  })

  it('assigns distinct visual kinds for failed / in_progress / completed', () => {
    expect(statusVisualKind('failed')).toBe('failed')
    expect(statusVisualKind('in_progress')).toBe('running')
    expect(statusVisualKind('completed')).toBe('passed')
    expect(statusVisualKind('cancelled')).toBe('cancelled')
    expect(statusVisualKind('open')).toBe('open')
    expect(statusVisualKind('closed')).toBe('passed')
  })

  it('emits stable CSS modifiers (not a single green badge class)', () => {
    expect(statusClass('failed')).toBe('status-failed')
    expect(statusClass('in_progress')).toBe('status-in_progress')
    expect(statusClass('completed')).toBe('status-completed')
    expect(statusClass('Passed')).toBe('status-passed')
    expect(new Set([
      statusClass('failed'),
      statusClass('in_progress'),
      statusClass('completed'),
    ]).size).toBe(3)
  })
})
