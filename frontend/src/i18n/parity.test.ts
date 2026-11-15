import { describe, expect, it } from 'vitest'
import { IDENTICAL_TO_EN_ALLOWLIST } from './identical-allowlist'
import { LOCALE_CODES, localeDictionaries } from './index'

describe('i18n locale parity', () => {
  const enKeys = Object.keys(localeDictionaries.en).sort()
  const en = localeDictionaries.en

  it('English dictionary is non-empty', () => {
    expect(enKeys.length).toBeGreaterThan(50)
  })

  it('identical-to-English allowlist only references known keys', () => {
    const unknown = [...IDENTICAL_TO_EN_ALLOWLIST].filter((k) => !enKeys.includes(k))
    expect(unknown).toEqual([])
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

    if (code === 'en') continue

    it(`${code} does not leave values identical to English unless allowlisted`, () => {
      const dict = localeDictionaries[code]
      const identical = enKeys.filter(
        (k) => dict[k] === en[k] && !IDENTICAL_TO_EN_ALLOWLIST.has(k),
      )
      expect(identical).toEqual([])
    })
  }
})
