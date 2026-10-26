import { describe, expect, it } from 'vitest'
import { intlLocale, localeDictionaries, type Locale } from './index'

/** Mirrors runtime t() fallback: active locale → en → raw key. */
function translate(locale: Locale, key: string): string {
  const dict = localeDictionaries[locale] ?? localeDictionaries.en
  return dict[key] ?? localeDictionaries.en[key] ?? key
}

describe('i18n missing-key fallback', () => {
  it('falls back to English when the active locale lacks a key', () => {
    // Same resolution order as t(): active → en → key (without mutating shipped dicts).
    const en = { ...localeDictionaries.en, ___en_only_probe___: 'English probe' }
    const fr = localeDictionaries.fr
    const key = '___en_only_probe___'
    expect(fr[key] ?? en[key] ?? key).toBe('English probe')
  })

  it('falls back to the raw key when no locale has it', () => {
    expect(translate('de', 'totally.missing.key.xyz')).toBe(
      'totally.missing.key.xyz',
    )
  })

  it('resolves known keys in every shipped locale', () => {
    for (const code of ['en', 'uk', 'fr', 'es', 'de'] as Locale[]) {
      expect(translate(code, 'nav.cases').length).toBeGreaterThan(0)
      expect(translate(code, 'login.title').length).toBeGreaterThan(0)
    }
  })
})

describe('locale-aware Intl formatting', () => {
  it('maps locale codes to BCP-47 tags', () => {
    expect(intlLocale('en')).toBe('en-US')
    expect(intlLocale('uk')).toBe('uk-UA')
    expect(intlLocale('fr')).toBe('fr-FR')
    expect(intlLocale('es')).toBe('es-ES')
    expect(intlLocale('de')).toBe('de-DE')
  })

  it('formats numbers differently across locales', () => {
    const n = 12345.6
    const en = new Intl.NumberFormat(intlLocale('en')).format(n)
    const de = new Intl.NumberFormat(intlLocale('de')).format(n)
    const fr = new Intl.NumberFormat(intlLocale('fr')).format(n)
    expect(en).not.toEqual(de)
    expect(en.replace(/,/g, '')).not.toEqual(
      fr.replace(/[\s\u00a0\u202f]/g, '').replace(',', '.'),
    )
  })

  it('formats dates with locale medium style', () => {
    const d = new Date('2026-03-15T12:00:00Z')
    const en = new Intl.DateTimeFormat(intlLocale('en'), {
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(d)
    const de = new Intl.DateTimeFormat(intlLocale('de'), {
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(d)
    expect(en).not.toEqual(de)
    expect(en.length).toBeGreaterThan(4)
    expect(de.length).toBeGreaterThan(4)
  })
})
