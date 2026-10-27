/**
 * Self-hosted UI fonts (no Google Fonts / third-party requests).
 * Body fonts: Inter (default), IBM Plex Sans, Noto Sans, Source Sans 3.
 * Always available: IBM Plex Mono (code), Fraunces (display headings).
 * Licenses: each @fontsource package LICENSE (OFL/SIL), also under
 * frontend/licenses/fonts/.
 */

export const UI_FONT_INTER = 'inter'
export const UI_FONT_IBM_PLEX_SANS = 'ibm_plex_sans'
export const UI_FONT_NOTO_SANS = 'noto_sans'
export const UI_FONT_SOURCE_SANS_3 = 'source_sans_3'

export const UI_FONT_FAMILIES = [
  UI_FONT_INTER,
  UI_FONT_IBM_PLEX_SANS,
  UI_FONT_NOTO_SANS,
  UI_FONT_SOURCE_SANS_3,
] as const

export type UiFontFamily = (typeof UI_FONT_FAMILIES)[number]

export const DEFAULT_UI_FONT: UiFontFamily = UI_FONT_INTER

const CSS_FAMILY: Record<UiFontFamily, string> = {
  inter: '"Inter", "Segoe UI", system-ui, sans-serif',
  ibm_plex_sans: '"IBM Plex Sans", "Segoe UI", system-ui, sans-serif',
  noto_sans: '"Noto Sans", "Segoe UI", system-ui, sans-serif',
  source_sans_3: '"Source Sans 3", "Segoe UI", system-ui, sans-serif',
}

const DISPLAY_STACK = '"Fraunces", "Iowan Old Style", Georgia, serif'
const MONO_STACK = '"IBM Plex Mono", ui-monospace, monospace'

const loaded = new Set<string>()

function normalizeFontFamily(raw: string | null | undefined): UiFontFamily {
  const f = (raw ?? DEFAULT_UI_FONT).trim().toLowerCase().replace(/-/g, '_')
  return (UI_FONT_FAMILIES as readonly string[]).includes(f)
    ? (f as UiFontFamily)
    : DEFAULT_UI_FONT
}

/** Load shared mono + display faces once (small). */
export async function ensureSharedFonts(): Promise<void> {
  if (!loaded.has('shared')) {
    await Promise.all([
      import('@fontsource/ibm-plex-mono/500.css'),
      import('@fontsource/ibm-plex-mono/600.css'),
      import('@fontsource/fraunces/500.css'),
      import('@fontsource/fraunces/600.css'),
    ])
    loaded.add('shared')
  }
}

/** Lazy-load only the selected body font (latin + cyrillic via unicode-range). */
export async function loadUiFont(family: string): Promise<UiFontFamily> {
  const id = normalizeFontFamily(family)
  await ensureSharedFonts()
  if (!loaded.has(id)) {
    switch (id) {
      case UI_FONT_INTER:
        await Promise.all([
          import('@fontsource/inter/400.css'),
          import('@fontsource/inter/600.css'),
        ])
        break
      case UI_FONT_IBM_PLEX_SANS:
        await Promise.all([
          import('@fontsource/ibm-plex-sans/400.css'),
          import('@fontsource/ibm-plex-sans/600.css'),
        ])
        break
      case UI_FONT_NOTO_SANS:
        await Promise.all([
          import('@fontsource/noto-sans/400.css'),
          import('@fontsource/noto-sans/600.css'),
        ])
        break
      case UI_FONT_SOURCE_SANS_3:
        await Promise.all([
          import('@fontsource/source-sans-3/400.css'),
          import('@fontsource/source-sans-3/600.css'),
        ])
        break
    }
    loaded.add(id)
  }
  return id
}

/** Apply CSS variables + data attr for the body font (after loadUiFont). */
export function applyUiFontVars(family: string): UiFontFamily {
  const id = normalizeFontFamily(family)
  const root = document.documentElement
  root.setAttribute('data-ui-font', id)
  root.style.setProperty('--font-body', CSS_FAMILY[id])
  root.style.setProperty('--font-display', DISPLAY_STACK)
  root.style.setProperty('--font-mono', MONO_STACK)
  return id
}

export { normalizeFontFamily, CSS_FAMILY }
