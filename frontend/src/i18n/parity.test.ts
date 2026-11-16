import { describe, expect, it } from 'vitest'
import { IDENTICAL_TO_EN_ALLOWLIST } from './identical-allowlist'
import {
  LATIN_IN_CYRILLIC_ALLOWLIST,
  latinTokensInValue,
} from './latin-allowlist'
import { LOCALE_CODES, localeDictionaries } from './index'

/** Locales written primarily in Cyrillic — Latin tokens need an allowlist. */
const CYRILLIC_LOCALES = new Set(['uk'])

describe('i18n locale parity', () => {
  const enKeys = Object.keys(localeDictionaries.en).sort()
  const en = localeDictionaries.en

  it('English dictionary is non-empty', () => {
    expect(enKeys.length).toBeGreaterThan(50)
  })

  it('identical-to-English allowlist only references known keys', () => {
    const unknown = [...IDENTICAL_TO_EN_ALLOWLIST].filter(
      (k) => !enKeys.includes(k),
    )
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

    it(`${code} placeholders match English`, () => {
      const dict = localeDictionaries[code]
      const mismatches: { key: string; en: string[]; locale: string[] }[] = []
      for (const key of enKeys) {
        const enPh = placeholders(String(en[key] ?? ''))
        const locPh = placeholders(String(dict[key] ?? ''))
        if (enPh.join(',') !== locPh.join(',')) {
          mismatches.push({ key, en: enPh, locale: locPh })
        }
      }
      expect(mismatches).toEqual([])
    })

    if (CYRILLIC_LOCALES.has(code)) {
      it(`${code} has no unexpected Latin-script words (allowlist brands/tech)`, () => {
        const dict = localeDictionaries[code]
        const offenders: { key: string; tokens: string[] }[] = []
        for (const key of enKeys) {
          const tokens = latinTokensInValue(String(dict[key] ?? '')).filter(
            (t) => !LATIN_IN_CYRILLIC_ALLOWLIST.has(t),
          )
          if (tokens.length) {
            offenders.push({ key, tokens: [...new Set(tokens)] })
          }
        }
        expect(offenders).toEqual([])
      })
    }
  }
})

function placeholders(value: string): string[] {
  const found: string[] = []
  const re = /\{([a-zA-Z_][a-zA-Z0-9_]*)\}/g
  let m: RegExpExecArray | null
  while ((m = re.exec(value))) found.push(m[1])
  return [...new Set(found)].sort()
}
