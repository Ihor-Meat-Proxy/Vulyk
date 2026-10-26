export type Locale = 'en' | 'uk' | 'fr' | 'es' | 'de'

export type Dict = Record<string, string>

export type LocaleMeta = {
  code: Locale
  /** Native language name shown in the language menu */
  nativeName: string
  /** BCP 47 tag for Intl date/number formatting */
  intl: string
}

export const LOCALES: LocaleMeta[] = [
  { code: 'en', nativeName: 'English', intl: 'en-US' },
  { code: 'uk', nativeName: 'Українська', intl: 'uk-UA' },
  { code: 'fr', nativeName: 'Français', intl: 'fr-FR' },
  { code: 'es', nativeName: 'Español', intl: 'es-ES' },
  { code: 'de', nativeName: 'Deutsch', intl: 'de-DE' },
]

export const LOCALE_CODES: Locale[] = LOCALES.map((l) => l.code)

export function normalizeLocale(value: string | null | undefined): Locale | null {
  if (!value) return null
  const base = value.toLowerCase().split(/[-_]/)[0]
  if ((LOCALE_CODES as string[]).includes(base)) return base as Locale
  return null
}

export function intlLocale(locale: Locale): string {
  return LOCALES.find((l) => l.code === locale)?.intl ?? 'en-US'
}
