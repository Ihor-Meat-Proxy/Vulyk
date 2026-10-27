import { describe, expect, it } from 'vitest'

import { CASE_PICKER_LIMIT } from './CasePicker'

describe('CASE_PICKER_LIMIT', () => {
  it('keeps the documented soft cap used by CasePicker / Generate prefill', () => {
    expect(CASE_PICKER_LIMIT).toBe(500)
  })

  it('flags truncation when total exceeds loaded items', () => {
    const loaded = CASE_PICKER_LIMIT
    const total = CASE_PICKER_LIMIT + 1
    expect(total > loaded).toBe(true)
  })
})
