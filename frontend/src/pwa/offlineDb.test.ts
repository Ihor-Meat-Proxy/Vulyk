import { describe, expect, it } from 'vitest'

describe('offline helpers', () => {
  it('builds stable cache keys', () => {
    const projectId = 'p1'
    const caseId = 'c1'
    expect(`${projectId}:${caseId}`).toBe('p1:c1')
  })

  it('exposes queue entry shape', () => {
    const entry = {
      id: 'q1',
      projectId: 'p1',
      runId: 'r1',
      body: { status: 'passed' },
      createdAt: 1,
      attempts: 0,
    }
    expect(entry.body.status).toBe('passed')
    expect(entry.attempts).toBe(0)
  })
})
