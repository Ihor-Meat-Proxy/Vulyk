import { describe, expect, it } from 'vitest'
import {
  assertSafeTestAttrs,
  findSuspiciousTestAttrValue,
  isSafeEntityKey,
} from './security'

describe('test attr security', () => {
  it('isSafeEntityKey accepts API keys and rejects spaces / emails', () => {
    expect(isSafeEntityKey('VQA-001')).toBe(true)
    expect(isSafeEntityKey('ci-mcp')).toBe(true)
    expect(isSafeEntityKey('Suite With Spaces')).toBe(false)
    expect(isSafeEntityKey('admin@vulyk.local')).toBe(false)
    expect(isSafeEntityKey('')).toBe(false)
    expect(isSafeEntityKey(null)).toBe(false)
  })

  it('allows static kebab test ids, UUIDs, case keys, indexes', () => {
    expect(
      findSuspiciousTestAttrValue('data-testid', 'login-email-input'),
    ).toBeNull()
    expect(
      findSuspiciousTestAttrValue(
        'data-entity-id',
        '550e8400-e29b-41d4-a716-446655440000',
      ),
    ).toBeNull()
    expect(findSuspiciousTestAttrValue('data-entity-key', 'VQA-001')).toBeNull()
    expect(findSuspiciousTestAttrValue('data-entity-key', 'KOC-12')).toBeNull()
    expect(findSuspiciousTestAttrValue('data-entity-key', 'en')).toBeNull()
    expect(findSuspiciousTestAttrValue('data-entity-key', 'ci-mcp')).toBeNull()
    expect(findSuspiciousTestAttrValue('data-index', '0')).toBeNull()
    expect(findSuspiciousTestAttrValue('data-depth', '2')).toBeNull()
  })

  it('rejects emails and @ in any test attr', () => {
    expect(
      findSuspiciousTestAttrValue('data-entity-key', 'admin@vulyk.local')
        ?.reason,
    ).toMatch(/@/)
    expect(
      findSuspiciousTestAttrValue('data-testid', 'user-admin@x.com')?.reason,
    ).toMatch(/@/)
  })

  it('rejects JWT / bearer / high-entropy secrets', () => {
    expect(
      findSuspiciousTestAttrValue(
        'data-entity-key',
        'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0In0.sig',
      )?.reason,
    ).toMatch(/JWT/i)
    expect(
      findSuspiciousTestAttrValue(
        'data-entity-id',
        'Bearer eyJhbGciOiJIUzI1NiJ9.e30.sig',
      )?.reason,
    ).toMatch(/Authorization|JWT|@|secret|Bearer/i)
    expect(
      findSuspiciousTestAttrValue(
        'data-entity-key',
        'a'.repeat(48),
      )?.reason,
    ).toMatch(/entropy|token|secret/i)
  })

  it('rejects interpolated / non-kebab test ids', () => {
    expect(
      findSuspiciousTestAttrValue(
        'data-testid',
        'folder-count-550e8400-e29b-41d4-a716-446655440000',
      )?.reason,
    ).toMatch(/UUID|kebab|PII|interpolat/i)
    expect(
      findSuspiciousTestAttrValue('data-testid', 'NotKebab')?.reason,
    ).toMatch(/kebab|PII/i)
  })

  it('assertSafeTestAttrs throws on suspicious values', () => {
    expect(() =>
      assertSafeTestAttrs({
        'data-testid': 'admin-user-row',
        'data-entity-key': 'user@example.com',
      }),
    ).toThrow(/Unsafe test attribute/)
    expect(() =>
      assertSafeTestAttrs({
        'data-testid': 'case-row',
        'data-entity-id': '550e8400-e29b-41d4-a716-446655440000',
        'data-entity-key': 'VQA-001',
        'data-index': '3',
      }),
    ).not.toThrow()
  })
})
