/**
 * Design tokens for Vulyk appearance themes.
 * Status colours follow Okabe–Ito (colour-blind safe).
 * Contrast: AA ≥4.5:1 for light/dark/sepia; AAA ≥7:1 for high_contrast.
 */

export type ThemeId = 'light' | 'dark' | 'high_contrast' | 'sepia'
export type ThemeChoice = 'system' | ThemeId
export type Density = 'compact' | 'comfortable'

export type ThemeTokens = {
  bg: string
  bgElevated: string
  ink: string
  inkMuted: string
  line: string
  accent: string
  accentSoft: string
  accentOn: string
  warn: string
  danger: string
  focusRing: string
  /** PWA / browser chrome */
  themeColor: string
  /** Status: Passed — bluish green */
  statusPassedFg: string
  statusPassedBg: string
  statusPassedSolid: string
  /** Failed — vermillion / orange-red */
  statusFailedFg: string
  statusFailedBg: string
  statusFailedSolid: string
  /** Blocked — purple */
  statusBlockedFg: string
  statusBlockedBg: string
  statusBlockedSolid: string
  /** Skipped — grey */
  statusSkippedFg: string
  statusSkippedBg: string
  statusSkippedSolid: string
  /** Retest — amber */
  statusRetestFg: string
  statusRetestBg: string
  statusRetestSolid: string
  /** Untested — outline / muted */
  statusUntestedFg: string
  statusUntestedBg: string
  statusUntestedBorder: string
  /** Ready — sky blue */
  statusReadyFg: string
  statusReadyBg: string
  statusReadySolid: string
  /** Running — blue */
  statusRunningFg: string
  statusRunningBg: string
  statusRunningSolid: string
  /** Untestable — slate / dashed */
  statusUntestableFg: string
  statusUntestableBg: string
  statusUntestableSolid: string
  codeBg: string
  codeInk: string
  surfaceInput: string
}

/** Okabe–Ito reference hues (adjusted per theme for contrast). */
export const OKABE_ITO = {
  bluishGreen: '#009E73',
  vermillion: '#D55E00',
  reddishPurple: '#CC79A7',
  orange: '#E69F00',
  skyBlue: '#56B4E9',
  blue: '#0072B2',
  yellow: '#F0E442',
  grey: '#7A7A7A',
} as const

export const THEME_TOKENS: Record<ThemeId, ThemeTokens> = {
  light: {
    bg: '#F7F8FA',
    bgElevated: '#EEF0F3',
    ink: '#1F2328',
    inkMuted: '#57606A',
    line: '#D0D7DE',
    accent: '#0B6E4F',
    accentSoft: '#D4EDE3',
    accentOn: '#F7F8FA',
    warn: '#9A6700',
    danger: '#B42318',
    focusRing: '#0B6E4F',
    themeColor: '#0B6E4F',
    statusPassedFg: '#006B4F',
    statusPassedBg: '#D4EDE3',
    statusPassedSolid: '#009E73',
    statusFailedFg: '#9A3B00',
    statusFailedBg: '#F6E0D2',
    statusFailedSolid: '#D55E00',
    statusBlockedFg: '#6B2F6E',
    statusBlockedBg: '#EED9EC',
    statusBlockedSolid: '#A15C9E',
    statusSkippedFg: '#4A4F57',
    statusSkippedBg: '#E4E6EA',
    statusSkippedSolid: '#7A7A7A',
    statusRetestFg: '#7A5200',
    statusRetestBg: '#F5E8C8',
    statusRetestSolid: '#E69F00',
    statusUntestedFg: '#57606A',
    statusUntestedBg: 'transparent',
    statusUntestedBorder: '#8B949E',
    statusReadyFg: '#0A5A8A',
    statusReadyBg: '#D6EBF8',
    statusReadySolid: '#1A7BB5',
    statusRunningFg: '#005285',
    statusRunningBg: '#D0E4F2',
    statusRunningSolid: '#0072B2',
    statusUntestableFg: '#4A5560',
    statusUntestableBg: '#E2E6EA',
    statusUntestableSolid: '#5A6270',
    codeBg: '#24292F',
    codeInk: '#E6EDF3',
    surfaceInput: '#EEF0F3',
  },
  dark: {
    bg: '#121417',
    bgElevated: '#1A1D21',
    ink: '#E6E6E6',
    inkMuted: '#A8ADB4',
    line: '#2E333A',
    accent: '#4DB896',
    accentSoft: '#1A332B',
    accentOn: '#0C1210',
    warn: '#D4A017',
    danger: '#E07070',
    focusRing: '#4DB896',
    themeColor: '#121417',
    statusPassedFg: '#6FD6B0',
    statusPassedBg: '#1A332B',
    statusPassedSolid: '#009E73',
    statusFailedFg: '#F0A070',
    statusFailedBg: '#3A2418',
    statusFailedSolid: '#D55E00',
    statusBlockedFg: '#D9A0D4',
    statusBlockedBg: '#2E1F2E',
    statusBlockedSolid: '#A15C9E',
    statusSkippedFg: '#B0B4BA',
    statusSkippedBg: '#2A2D32',
    statusSkippedSolid: '#8A8A8A',
    statusRetestFg: '#E8C46A',
    statusRetestBg: '#33280F',
    statusRetestSolid: '#E69F00',
    statusUntestedFg: '#A8ADB4',
    statusUntestedBg: 'transparent',
    statusUntestedBorder: '#6E7681',
    statusReadyFg: '#8FCBE8',
    statusReadyBg: '#1A2E3A',
    statusReadySolid: '#56B4E9',
    statusRunningFg: '#7AB8DC',
    statusRunningBg: '#152838',
    statusRunningSolid: '#0072B2',
    statusUntestableFg: '#B0B4BA',
    statusUntestableBg: '#2A2D32',
    statusUntestableSolid: '#8A8A8A',
    codeBg: '#0D0F12',
    codeInk: '#E6E6E6',
    surfaceInput: '#1A1D21',
  },
  high_contrast: {
    bg: '#000000',
    bgElevated: '#0A0A0A',
    ink: '#FFFFFF',
    inkMuted: '#E0E0E0',
    line: '#FFFFFF',
    accent: '#00FFC8',
    accentSoft: '#003D30',
    accentOn: '#000000',
    warn: '#FFD100',
    danger: '#FF6B4A',
    focusRing: '#FFFF00',
    themeColor: '#000000',
    statusPassedFg: '#00FFC8',
    statusPassedBg: '#002820',
    statusPassedSolid: '#00FFC8',
    statusFailedFg: '#FFB199',
    statusFailedBg: '#2A0800',
    statusFailedSolid: '#FF6B4A',
    statusBlockedFg: '#FF9EE8',
    statusBlockedBg: '#2A0A28',
    statusBlockedSolid: '#FF9EE8',
    statusSkippedFg: '#E0E0E0',
    statusSkippedBg: '#1A1A1A',
    statusSkippedSolid: '#C0C0C0',
    statusRetestFg: '#FFD100',
    statusRetestBg: '#2A2200',
    statusRetestSolid: '#FFD100',
    statusUntestedFg: '#FFFFFF',
    statusUntestedBg: 'transparent',
    statusUntestedBorder: '#FFFFFF',
    statusReadyFg: '#7AD0FF',
    statusReadyBg: '#002438',
    statusReadySolid: '#7AD0FF',
    statusRunningFg: '#66B8FF',
    statusRunningBg: '#001A30',
    statusRunningSolid: '#66B8FF',
    statusUntestableFg: '#E0E0E0',
    statusUntestableBg: '#1A1A1A',
    statusUntestableSolid: '#C0C0C0',
    codeBg: '#000000',
    codeInk: '#FFFFFF',
    surfaceInput: '#0A0A0A',
  },
  sepia: {
    bg: '#1A1612',
    bgElevated: '#241E18',
    ink: '#E8DCC8',
    inkMuted: '#B8A890',
    line: '#3D342A',
    accent: '#C4893A',
    accentSoft: '#3A2A18',
    accentOn: '#1A1612',
    warn: '#D4A04A',
    danger: '#D07050',
    focusRing: '#C4893A',
    themeColor: '#1A1612',
    statusPassedFg: '#7BC4A0',
    statusPassedBg: '#1E2E26',
    statusPassedSolid: '#3D9B74',
    statusFailedFg: '#E09060',
    statusFailedBg: '#3A2418',
    statusFailedSolid: '#C45A20',
    statusBlockedFg: '#C898C0',
    statusBlockedBg: '#2E2228',
    statusBlockedSolid: '#9A6A90',
    statusSkippedFg: '#B0A898',
    statusSkippedBg: '#2A2620',
    statusSkippedSolid: '#8A8070',
    statusRetestFg: '#E0B860',
    statusRetestBg: '#332818',
    statusRetestSolid: '#C4893A',
    statusUntestedFg: '#B8A890',
    statusUntestedBg: 'transparent',
    statusUntestedBorder: '#6E6458',
    statusReadyFg: '#7AB0C8',
    statusReadyBg: '#1E2A30',
    statusReadySolid: '#4A90B0',
    statusRunningFg: '#6A9CBC',
    statusRunningBg: '#1A2630',
    statusRunningSolid: '#3A7090',
    statusUntestableFg: '#B0A898',
    statusUntestableBg: '#2A2620',
    statusUntestableSolid: '#8A8070',
    codeBg: '#120F0C',
    codeInk: '#E8DCC8',
    surfaceInput: '#241E18',
  },
}

/** Text/background pairs that must meet contrast minimums. */
export const CONTRAST_PAIRS: Array<{
  fg: keyof ThemeTokens
  bg: keyof ThemeTokens
  label: string
}> = [
  { fg: 'ink', bg: 'bg', label: 'body text' },
  { fg: 'inkMuted', bg: 'bg', label: 'muted text' },
  { fg: 'ink', bg: 'bgElevated', label: 'elevated body' },
  { fg: 'accentOn', bg: 'accent', label: 'accent button' },
  { fg: 'statusPassedFg', bg: 'statusPassedBg', label: 'passed badge' },
  { fg: 'statusFailedFg', bg: 'statusFailedBg', label: 'failed badge' },
  { fg: 'statusBlockedFg', bg: 'statusBlockedBg', label: 'blocked badge' },
  { fg: 'statusSkippedFg', bg: 'statusSkippedBg', label: 'skipped badge' },
  { fg: 'statusRetestFg', bg: 'statusRetestBg', label: 'retest badge' },
  { fg: 'statusUntestedFg', bg: 'bg', label: 'untested label' },
  { fg: 'statusReadyFg', bg: 'statusReadyBg', label: 'ready badge' },
  { fg: 'statusRunningFg', bg: 'statusRunningBg', label: 'running badge' },
  { fg: 'statusUntestableFg', bg: 'statusUntestableBg', label: 'untestable badge' },
  { fg: 'codeInk', bg: 'codeBg', label: 'code block' },
]

export const FONT_SIZE_STEPS = [87, 100, 112, 125, 138] as const
export type FontSizeStep = (typeof FONT_SIZE_STEPS)[number]

export type AppearancePrefs = {
  theme: ThemeChoice
  fontSize: number
  density: Density
  /** null = follow prefers-reduced-motion */
  reducedMotion: boolean | null
}

export const DEFAULT_APPEARANCE: AppearancePrefs = {
  theme: 'system',
  fontSize: 100,
  density: 'comfortable',
  reducedMotion: null,
}

export const APPEARANCE_STORAGE_KEY = 'vulyk.appearance'

export function clampFontSize(n: number): number {
  const nearest = FONT_SIZE_STEPS.reduce((best, step) =>
    Math.abs(step - n) < Math.abs(best - n) ? step : best,
  )
  return nearest
}

/** Relative luminance (sRGB), WCAG 2.x. */
export function relativeLuminance(hex: string): number {
  const c = hex.replace('#', '')
  if (c.length !== 6) {
    throw new Error(`expected #RRGGBB, got ${hex}`)
  }
  const rgb = [0, 1, 2].map((i) => {
    const v = parseInt(c.slice(i * 2, i * 2 + 2), 16) / 255
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4
  })
  return 0.2126 * rgb[0]! + 0.7152 * rgb[1]! + 0.0722 * rgb[2]!
}

export function contrastRatio(fg: string, bg: string): number {
  if (fg === 'transparent' || bg === 'transparent') {
    // Transparent backgrounds are measured against page bg by callers.
    return Infinity
  }
  const L1 = relativeLuminance(fg)
  const L2 = relativeLuminance(bg)
  const lighter = Math.max(L1, L2)
  const darker = Math.min(L1, L2)
  return (lighter + 0.05) / (darker + 0.05)
}

export function resolveThemeChoice(
  choice: ThemeChoice,
  opts: {
    prefersDark?: boolean
    prefersContrast?: boolean
  } = {},
): ThemeId {
  if (choice !== 'system') return choice
  if (opts.prefersContrast) return 'high_contrast'
  return opts.prefersDark ? 'dark' : 'light'
}

export function tokensToCssVars(tokens: ThemeTokens): Record<string, string> {
  return {
    '--bg': tokens.bg,
    '--bg-elevated': tokens.bgElevated,
    '--ink': tokens.ink,
    '--ink-muted': tokens.inkMuted,
    '--line': tokens.line,
    '--accent': tokens.accent,
    '--accent-soft': tokens.accentSoft,
    '--accent-on': tokens.accentOn,
    '--warn': tokens.warn,
    '--danger': tokens.danger,
    '--focus-ring': tokens.focusRing,
    '--theme-color': tokens.themeColor,
    '--status-passed-fg': tokens.statusPassedFg,
    '--status-passed-bg': tokens.statusPassedBg,
    '--status-passed': tokens.statusPassedSolid,
    '--status-failed-fg': tokens.statusFailedFg,
    '--status-failed-bg': tokens.statusFailedBg,
    '--status-failed': tokens.statusFailedSolid,
    '--status-blocked-fg': tokens.statusBlockedFg,
    '--status-blocked-bg': tokens.statusBlockedBg,
    '--status-blocked': tokens.statusBlockedSolid,
    '--status-skipped-fg': tokens.statusSkippedFg,
    '--status-skipped-bg': tokens.statusSkippedBg,
    '--status-skipped': tokens.statusSkippedSolid,
    '--status-retest-fg': tokens.statusRetestFg,
    '--status-retest-bg': tokens.statusRetestBg,
    '--status-retest': tokens.statusRetestSolid,
    '--status-untested-fg': tokens.statusUntestedFg,
    '--status-untested-bg': tokens.statusUntestedBg,
    '--status-untested-border': tokens.statusUntestedBorder,
    '--status-ready-fg': tokens.statusReadyFg,
    '--status-ready-bg': tokens.statusReadyBg,
    '--status-ready': tokens.statusReadySolid,
    '--status-running-fg': tokens.statusRunningFg,
    '--status-running-bg': tokens.statusRunningBg,
    '--status-running': tokens.statusRunningSolid,
    '--status-untestable-fg': tokens.statusUntestableFg,
    '--status-untestable-bg': tokens.statusUntestableBg,
    '--status-untestable': tokens.statusUntestableSolid,
    '--code-bg': tokens.codeBg,
    '--code-ink': tokens.codeInk,
    '--surface-input': tokens.surfaceInput,
  }
}
