import { describe, expect, it } from 'vitest'
import { formatTimestamp } from './timezone'

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
})
