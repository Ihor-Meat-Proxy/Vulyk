import { describe, expect, it } from 'vitest'
import { enumLabel } from './enumLabel'
import { localeDictionaries, type Locale } from './index'

/** Mirrors runtime t(): active locale → en → raw key. */
function makeT(locale: Locale) {
  const dict = localeDictionaries[locale] ?? localeDictionaries.en
  return (key: string) => dict[key] ?? localeDictionaries.en[key] ?? key
}

describe('enumLabel', () => {
  it('translates priority and case status in every shipped locale', () => {
    for (const code of ['en', 'uk', 'fr', 'es', 'de'] as Locale[]) {
      const t = makeT(code)
      expect(enumLabel(t, 'priority', 'critical')).not.toBe('critical')
      expect(enumLabel(t, 'priority', 'medium')).not.toBe('medium')
      expect(enumLabel(t, 'status', 'ready')).not.toBe('ready')
      expect(enumLabel(t, 'status', 'draft')).not.toBe('draft')
      expect(enumLabel(t, 'status', 'deprecated')).not.toBe('deprecated')
    }
  })

  it('translates Ukrainian priority/status away from English raw values', () => {
    const t = makeT('uk')
    expect(enumLabel(t, 'priority', 'critical')).toBe('Критичний')
    expect(enumLabel(t, 'status', 'ready')).toBe('Готово')
  })

  it('lowercases result statuses like Passed → status.passed', () => {
    const t = makeT('en')
    expect(enumLabel(t, 'status', 'Passed')).toBe('Passed')
    expect(enumLabel(t, 'status', 'Failed')).toBe('Failed')
    const uk = makeT('uk')
    expect(enumLabel(uk, 'status', 'Passed')).toBe('Пройдено')
    expect(enumLabel(uk, 'status', 'Failed')).toBe('Провалено')
  })

  it('falls back to the raw value for unknown keys', () => {
    const t = makeT('en')
    expect(enumLabel(t, 'priority', 'urgent-xyz')).toBe('urgent-xyz')
    expect(enumLabel(t, 'status', 'mystery')).toBe('mystery')
  })
})
