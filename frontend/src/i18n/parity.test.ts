import { describe, expect, it } from 'vitest'
import { LOCALE_CODES, localeDictionaries } from './index'

describe('i18n locale parity', () => {
  const enKeys = Object.keys(localeDictionaries.en).sort()

  it('English dictionary is non-empty', () => {
    expect(enKeys.length).toBeGreaterThan(50)
  })

  for (const code of LOCALE_CODES) {
    it(`${code} has the same keys as en (no missing, no extras)`, () => {
      const keys = Object.keys(localeDictionaries[code]).sort()
      const missing = enKeys.filter((k) => !keys.includes(k))
      const extra = keys.filter((k) => !enKeys.includes(k))
      expect({ missing, extra }).toEqual({ missing: [], extra: [] })
    })

    it(`${code} has no empty translation values`, () => {
      const empty = Object.entries(localeDictionaries[code])
        .filter(([, v]) => !v || !String(v).trim())
        .map(([k]) => k)
      expect(empty).toEqual([])
    })
  }
})
