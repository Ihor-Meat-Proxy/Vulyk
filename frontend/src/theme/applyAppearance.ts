import {
  APPEARANCE_STORAGE_KEY,
  DEFAULT_APPEARANCE,
  THEME_TOKENS,
  clampFontSize,
  resolveThemeChoice,
  tokensToCssVars,
  type AppearancePrefs,
  type ThemeChoice,
  type ThemeId,
} from './tokens'

export function readStoredAppearance(): AppearancePrefs {
  try {
    const raw = localStorage.getItem(APPEARANCE_STORAGE_KEY)
    if (!raw) return { ...DEFAULT_APPEARANCE }
    const parsed = JSON.parse(raw) as Partial<AppearancePrefs>
    return normalizePrefs(parsed)
  } catch {
    return { ...DEFAULT_APPEARANCE }
  }
}

export function writeStoredAppearance(prefs: AppearancePrefs): void {
  try {
    localStorage.setItem(APPEARANCE_STORAGE_KEY, JSON.stringify(prefs))
  } catch {
    /* ignore quota / private mode */
  }
}

export function normalizePrefs(
  partial: Partial<AppearancePrefs> & {
    font_size?: number
    reduced_motion?: boolean | null
  },
): AppearancePrefs {
  const theme = (partial.theme ?? DEFAULT_APPEARANCE.theme) as ThemeChoice
  const validThemes: ThemeChoice[] = [
    'system',
    'light',
    'dark',
    'high_contrast',
    'sepia',
  ]
  return {
    theme: validThemes.includes(theme) ? theme : 'system',
    fontSize: clampFontSize(
      partial.fontSize ?? partial.font_size ?? DEFAULT_APPEARANCE.fontSize,
    ),
    density:
      partial.density === 'compact' || partial.density === 'comfortable'
        ? partial.density
        : DEFAULT_APPEARANCE.density,
    reducedMotion:
      partial.reducedMotion !== undefined
        ? partial.reducedMotion
        : partial.reduced_motion !== undefined
          ? partial.reduced_motion
          : DEFAULT_APPEARANCE.reducedMotion,
  }
}

export function mediaPrefs(): {
  prefersDark: boolean
  prefersContrast: boolean
  prefersReducedMotion: boolean
} {
  if (typeof window === 'undefined' || !window.matchMedia) {
    return {
      prefersDark: false,
      prefersContrast: false,
      prefersReducedMotion: false,
    }
  }
  return {
    prefersDark: window.matchMedia('(prefers-color-scheme: dark)').matches,
    prefersContrast: window.matchMedia('(prefers-contrast: more)').matches,
    prefersReducedMotion: window.matchMedia('(prefers-reduced-motion: reduce)')
      .matches,
  }
}

export function resolveReducedMotion(
  pref: boolean | null,
  systemPrefers: boolean,
): boolean {
  if (pref === null) return systemPrefers
  return pref
}

/** Apply appearance to <html> — CSS vars, data attrs, theme-color meta. */
export function applyAppearance(prefs: AppearancePrefs): ThemeId {
  const media = mediaPrefs()
  const resolved = resolveThemeChoice(prefs.theme, {
    prefersDark: media.prefersDark,
    prefersContrast: media.prefersContrast,
  })
  const tokens = THEME_TOKENS[resolved]
  const root = document.documentElement

  root.setAttribute('data-theme', resolved)
  root.setAttribute('data-theme-choice', prefs.theme)
  root.setAttribute('data-density', prefs.density)
  root.style.setProperty('--font-scale', String(prefs.fontSize / 100))

  const vars = tokensToCssVars(tokens)
  for (const [key, value] of Object.entries(vars)) {
    root.style.setProperty(key, value)
  }

  const reduce = resolveReducedMotion(
    prefs.reducedMotion,
    media.prefersReducedMotion,
  )
  if (reduce) {
    root.setAttribute('data-reduced-motion', 'true')
  } else {
    root.removeAttribute('data-reduced-motion')
  }

  const meta = document.querySelector('meta[name="theme-color"]')
  if (meta) {
    meta.setAttribute('content', tokens.themeColor)
  }

  root.style.colorScheme =
    resolved === 'light' || resolved === 'high_contrast'
      ? resolved === 'high_contrast'
        ? 'dark'
        : 'light'
      : 'dark'

  return resolved
}

export function prefsFromApi(userPrefs: {
  theme?: string
  font_size?: number
  density?: string
  reduced_motion?: boolean | null
} | null | undefined): AppearancePrefs {
  if (!userPrefs) return { ...DEFAULT_APPEARANCE }
  return normalizePrefs({
    theme: userPrefs.theme as ThemeChoice | undefined,
    font_size: userPrefs.font_size,
    density: userPrefs.density as AppearancePrefs['density'] | undefined,
    reduced_motion: userPrefs.reduced_motion ?? null,
  })
}

export function prefsToApi(prefs: AppearancePrefs) {
  return {
    theme: prefs.theme,
    font_size: prefs.fontSize,
    density: prefs.density,
    reduced_motion: prefs.reducedMotion,
  }
}
