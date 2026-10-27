import { describe, expect, it } from 'vitest'
import {
  DEFAULT_UI_FONT,
  UI_FONT_FAMILIES,
  normalizeFontFamily,
} from './fonts'
import { normalizePrefs } from './applyAppearance'

describe('ui fonts', () => {
  it('defaults invalid or missing family to Inter', () => {
    expect(normalizeFontFamily(undefined)).toBe(DEFAULT_UI_FONT)
    expect(normalizeFontFamily('')).toBe('inter')
    expect(normalizeFontFamily('Comic Sans')).toBe('inter')
    expect(normalizeFontFamily('ibm-plex-sans')).toBe('ibm_plex_sans')
  })

  it('lists the four selectable body fonts', () => {
    expect([...UI_FONT_FAMILIES]).toEqual([
      'inter',
      'ibm_plex_sans',
      'noto_sans',
      'source_sans_3',
    ])
  })

  it('normalizePrefs maps font_family from API and falls back', () => {
    expect(normalizePrefs({ font_family: 'noto_sans' }).fontFamily).toBe(
      'noto_sans',
    )
    expect(normalizePrefs({}).fontFamily).toBe('inter')
    expect(normalizePrefs({ fontFamily: 'nope' as 'inter' }).fontFamily).toBe(
      'inter',
    )
  })
})
