import { describe, expect, it } from 'vitest'
import { formatTimestamp, normalizeTimeZone } from './timezone'

describe('normalizeTimeZone', () => {
  it('maps Europe/Kiev to Europe/Kyiv', () => {
    expect(normalizeTimeZone('Europe/Kiev')).toBe('Europe/Kyiv')
    expect(normalizeTimeZone(' Europe/Kiev ')).toBe('Europe/Kyiv')
  })

  it('passes through modern aliases', () => {
    expect(normalizeTimeZone('Europe/Kyiv')).toBe('Europe/Kyiv')
    expect(normalizeTimeZone('UTC')).toBe('UTC')
  })
})

describe('formatTimestamp', () => {
  it('returns em dash for empty values', () => {
    expect(formatTimestamp(null)).toBe('—')
    expect(formatTimestamp('')).toBe('—')
  })

  it('formats with an explicit timezone', () => {
    const text = formatTimestamp('2024-01-15T12:00:00.000Z', {
      timeZone: 'UTC',
      locale: 'en-US',
      dateStyle: 'medium',
      timeStyle: 'short',
    })
    expect(text).toMatch(/2024/)
    expect(text).not.toBe('—')
  })

  it('uses Kyiv when Kiev is requested', () => {
    const text = formatTimestamp('2024-06-15T12:00:00.000Z', {
      timeZone: 'Europe/Kiev',
      locale: 'en-GB',
      dateStyle: 'medium',
      timeStyle: 'short',
    })
    expect(text).toMatch(/2024/)
    expect(text).not.toBe('—')
  })
})
