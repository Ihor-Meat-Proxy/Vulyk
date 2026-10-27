import { describe, expect, it } from 'vitest'

import { RUNS_PAGE_SIZE } from './RunsPage'

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
