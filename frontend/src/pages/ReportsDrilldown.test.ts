import { describe, expect, it } from 'vitest'

/** Mirrors backend comparison() for UI badge keys. */
function comparison(prev: string | null | undefined, curr: string): string | null {
  const currL = curr.toLowerCase()
  const prevL = prev?.toLowerCase()
  const decided = (s: string) => s === 'passed' || s === 'failed' || s === 'error'
  if (prevL === 'passed' && (currL === 'failed' || currL === 'error')) return 'newly_failing'
  if ((prevL === 'failed' || prevL === 'error') && currL === 'passed') return 'fixed'
  if (prevL && prevL !== currL && decided(prevL) && decided(currL)) return 'flaky'
  if (prevL && prevL === currL) return 'unchanged'
  return null
}

describe('run report comparison indicators', () => {
  it('detects newly failing and fixed', () => {
    expect(comparison('Passed', 'Failed')).toBe('newly_failing')
    expect(comparison('failed', 'passed')).toBe('fixed')
    expect(comparison('passed', 'passed')).toBe('unchanged')
    expect(comparison('error', 'passed')).toBe('fixed')
  })

  it('builds run report deep links with status pre-filter', () => {
    const runId = '11111111-1111-1111-1111-111111111111'
    const path = `/reports/runs/${runId}?status=failed`
    expect(path).toContain(runId)
    expect(new URLSearchParams(path.split('?')[1]).get('status')).toBe('failed')
  })
})
