import { describe, expect, it } from 'vitest'
import {
  CONTRAST_PAIRS,
  THEME_TOKENS,
  contrastRatio,
  type ThemeId,
} from './tokens'

const AA_MIN = 4.5
const AAA_MIN = 7.0

const THEMES = Object.keys(THEME_TOKENS) as ThemeId[]

describe('theme token contrast', () => {
  for (const theme of THEMES) {
    const min = theme === 'high_contrast' ? AAA_MIN : AA_MIN
    describe(`${theme} (≥${min}:1)`, () => {
      for (const pair of CONTRAST_PAIRS) {
        it(`${pair.label}: ${pair.fg} on ${pair.bg}`, () => {
          const tokens = THEME_TOKENS[theme]
          let fg = tokens[pair.fg]
          let bg = tokens[pair.bg]
          // Untested uses transparent bg — measure against page bg.
          if (bg === 'transparent') bg = tokens.bg
          if (fg === 'transparent') fg = tokens.ink
          const ratio = contrastRatio(fg, bg)
          expect(
            ratio,
            `${theme} ${pair.label}: ${fg} on ${bg} = ${ratio.toFixed(2)}:1 (need ${min})`,
          ).toBeGreaterThanOrEqual(min)
        })
      }
    })
  }

  it('solid status colours are distinguishable from page background', () => {
    for (const theme of THEMES) {
      const t = THEME_TOKENS[theme]
      // Chart fills are large UI + patterned; require ≥3:1 except amber on light
      // (Okabe–Ito orange is intentionally vivid — always paired with legend/label).
      for (const [name, solid] of [
        ['passed', t.statusPassedSolid],
        ['failed', t.statusFailedSolid],
        ['blocked', t.statusBlockedSolid],
        ['skipped', t.statusSkippedSolid],
        ['retest', t.statusRetestSolid],
      ] as const) {
        const ratio = contrastRatio(solid, t.bg)
        const min =
          theme === 'high_contrast'
            ? AAA_MIN
            : name === 'retest' && theme === 'light'
              ? 2.0
              : 3.0
        expect(
          ratio,
          `${theme} ${name} solid ${solid} on ${t.bg} = ${ratio.toFixed(2)}`,
        ).toBeGreaterThanOrEqual(min)
      }
    }
  })
})
