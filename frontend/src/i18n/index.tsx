import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react'
import { api } from '../api'
import { useAuth } from '../auth'
import de from './locales/de'
import en from './locales/en'
import es from './locales/es'
import fr from './locales/fr'
import uk from './locales/uk'
import {
  intlLocale,
  normalizeLocale,
  type Locale,
  type Dict,
} from './types'

export type { Locale, Dict, LocaleMeta } from './types'
export { LOCALES, LOCALE_CODES, normalizeLocale, intlLocale } from './types'

const STORAGE_KEY = 'vulyk.locale'

const dictionaries: Record<Locale, Dict> = { en, uk, fr, es, de }

function readStoredLocale(): Locale | null {
  try {
    return normalizeLocale(localStorage.getItem(STORAGE_KEY))
  } catch {
    return null
  }
}

function writeStoredLocale(locale: Locale) {
  try {
    localStorage.setItem(STORAGE_KEY, locale)
  } catch {
    /* ignore */
  }
}

type I18nContextValue = {
  locale: Locale
  intl: string
  setLocale: (locale: Locale) => Promise<void>
  t: (key: string, vars?: Record<string, string | number>) => string
  formatNumber: (value: number, opts?: Intl.NumberFormatOptions) => string
  formatDate: (
    value: string | Date | null | undefined,
    opts?: Intl.DateTimeFormatOptions,
  ) => string
}

const I18nContext = createContext<I18nContextValue | null>(null)

function resolveInitialLocale(
  userLocale: string | null | undefined,
  defaultLocale: string | null | undefined,
): Locale {
  return (
    normalizeLocale(userLocale) ??
    readStoredLocale() ??
    normalizeLocale(defaultLocale) ??
    'en'
  )
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const { user, status, refreshStatus } = useAuth()
  const [locale, setLocaleState] = useState<Locale>(() =>
    resolveInitialLocale(user?.locale, status?.default_locale),
  )

  useEffect(() => {
    setLocaleState(
      resolveInitialLocale(user?.locale, status?.default_locale),
    )
  }, [user?.locale, status?.default_locale])

  useEffect(() => {
    if (typeof document !== 'undefined') {
      document.documentElement.lang = locale
    }
  }, [locale])

  const setLocale = useCallback(
    async (next: Locale) => {
      setLocaleState(next)
      writeStoredLocale(next)
      if (user) {
        try {
          await api.updateMe({ locale: next })
          await refreshStatus()
        } catch {
          /* keep local preference even if PATCH fails */
        }
      }
    },
    [user, refreshStatus],
  )

  const t = useCallback(
    (key: string, vars?: Record<string, string | number>) => {
      const dict = dictionaries[locale] ?? dictionaries.en
      let text = dict[key] ?? dictionaries.en[key] ?? key
      if (vars) {
        for (const [k, v] of Object.entries(vars)) {
          text = text.replaceAll(`{${k}}`, String(v))
        }
      }
      return text
    },
    [locale],
  )

  const intl = intlLocale(locale)

  const formatNumber = useCallback(
    (value: number, opts?: Intl.NumberFormatOptions) => {
      try {
        return new Intl.NumberFormat(intl, opts).format(value)
      } catch {
        return String(value)
      }
    },
    [intl],
  )

  const formatDate = useCallback(
    (
      value: string | Date | null | undefined,
      opts?: Intl.DateTimeFormatOptions,
    ) => {
      if (value == null || value === '') return '—'
      const date = value instanceof Date ? value : new Date(value)
      if (Number.isNaN(date.getTime())) return '—'
      try {
        return new Intl.DateTimeFormat(intl, {
          dateStyle: 'medium',
          timeStyle: 'short',
          ...opts,
        }).format(date)
      } catch {
        return date.toLocaleString()
      }
    },
    [intl],
  )

  const value = useMemo(
    () => ({ locale, intl, setLocale, t, formatNumber, formatDate }),
    [locale, intl, setLocale, t, formatNumber, formatDate],
  )

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
}

export function useI18n(): I18nContextValue {
  const ctx = useContext(I18nContext)
  if (!ctx) {
    throw new Error('useI18n must be used within I18nProvider')
  }
  return ctx
}

/** Exported for parity tests / tooling — do not mutate. */
export const localeDictionaries = dictionaries
