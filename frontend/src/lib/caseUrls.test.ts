import { describe, expect, it } from 'vitest'
import {
  casePath,
  isCaseKey,
  isCaseUuid,
  normalizeCaseKey,
} from './caseUrls'

describe('caseUrls', () => {
  it('detects UUIDs', () => {
    expect(isCaseUuid('550e8400-e29b-41d4-a716-446655440000')).toBe(true)
    expect(isCaseUuid('PLTK-2194')).toBe(false)
    expect(isCaseUuid(undefined)).toBe(false)
  })

  it('detects case keys', () => {
    expect(isCaseKey('PLTK-2194')).toBe(true)
    expect(isCaseKey('pltk-2194')).toBe(true)
    expect(isCaseKey('VQA-001')).toBe(true)
    expect(isCaseKey('550e8400-e29b-41d4-a716-446655440000')).toBe(false)
  })

  it('builds encoded case paths', () => {
    expect(casePath('PLTK-2194')).toBe('/cases/PLTK-2194')
  })

  it('normalizes keys to uppercase', () => {
    expect(normalizeCaseKey('  pltk-12 ')).toBe('PLTK-12')
  })
})
