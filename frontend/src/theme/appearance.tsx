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
import {
  applyAppearance,
  prefsFromApi,
  prefsToApi,
  readStoredAppearance,
  writeStoredAppearance,
} from './applyAppearance'
import {
  DEFAULT_APPEARANCE,
  type AppearancePrefs,
  type Density,
  type ThemeChoice,
} from './tokens'

type AppearanceContextValue = {
  prefs: AppearancePrefs
  setTheme: (theme: ThemeChoice) => Promise<void>
  setFontSize: (fontSize: number) => Promise<void>
  setDensity: (density: Density) => Promise<void>
  setReducedMotion: (value: boolean | null) => Promise<void>
  updatePrefs: (patch: Partial<AppearancePrefs>) => Promise<void>
}

const AppearanceContext = createContext<AppearanceContextValue | null>(null)

export function AppearanceProvider({ children }: { children: ReactNode }) {
  const { user, status, refreshStatus } = useAuth()
  const [prefs, setPrefsState] = useState<AppearancePrefs>(() =>
    readStoredAppearance(),
  )

  // Sync from user / instance default when auth status lands.
  useEffect(() => {
    if (user?.preferences) {
      const next = prefsFromApi(user.preferences)
      setPrefsState(next)
      writeStoredAppearance(next)
      applyAppearance(next)
      return
    }
    if (status?.default_theme) {
      const stored = readStoredAppearance()
      // Only apply instance default when local storage still has system default
      // and the user has never customized (theme still system is OK).
      if (!user) {
        const next: AppearancePrefs = {
          ...stored,
          theme: (status.default_theme as ThemeChoice) || stored.theme,
        }
        // Pre-login: keep localStorage overrides; fall back to instance theme
        // only when storage was empty / default system and instance differs.
        const hadStored = (() => {
          try {
            return Boolean(localStorage.getItem('vulyk.appearance'))
          } catch {
            return false
          }
        })()
        if (!hadStored && status.default_theme !== 'system') {
          setPrefsState(next)
          applyAppearance(next)
        }
      }
    }
  }, [user?.preferences, user?.id, status?.default_theme, user])

  // Apply + listen for system preference changes when theme is "system".
  useEffect(() => {
    applyAppearance(prefs)
    if (prefs.theme !== 'system' && prefs.reducedMotion !== null) return

    const mqDark = window.matchMedia('(prefers-color-scheme: dark)')
    const mqContrast = window.matchMedia('(prefers-contrast: more)')
    const mqMotion = window.matchMedia('(prefers-reduced-motion: reduce)')
    const onChange = () => applyAppearance(prefs)
    mqDark.addEventListener('change', onChange)
    mqContrast.addEventListener('change', onChange)
    mqMotion.addEventListener('change', onChange)
    return () => {
      mqDark.removeEventListener('change', onChange)
      mqContrast.removeEventListener('change', onChange)
      mqMotion.removeEventListener('change', onChange)
    }
  }, [prefs])

  const persist = useCallback(
    async (next: AppearancePrefs) => {
      setPrefsState(next)
      writeStoredAppearance(next)
      applyAppearance(next)
      if (user) {
        try {
          await api.updateMe({ preferences: prefsToApi(next) })
          await refreshStatus()
        } catch {
          /* keep local preference if PATCH fails */
        }
      }
    },
    [user, refreshStatus],
  )

  const updatePrefs = useCallback(
    async (patch: Partial<AppearancePrefs>) => {
      await persist({ ...prefs, ...patch })
    },
    [prefs, persist],
  )

  const setTheme = useCallback(
    (theme: ThemeChoice) => updatePrefs({ theme }),
    [updatePrefs],
  )
  const setFontSize = useCallback(
    (fontSize: number) => updatePrefs({ fontSize }),
    [updatePrefs],
  )
  const setDensity = useCallback(
    (density: Density) => updatePrefs({ density }),
    [updatePrefs],
  )
  const setReducedMotion = useCallback(
    (reducedMotion: boolean | null) => updatePrefs({ reducedMotion }),
    [updatePrefs],
  )

  const value = useMemo(
    () => ({
      prefs,
      setTheme,
      setFontSize,
      setDensity,
      setReducedMotion,
      updatePrefs,
    }),
    [prefs, setTheme, setFontSize, setDensity, setReducedMotion, updatePrefs],
  )

  return (
    <AppearanceContext.Provider value={value}>
      {children}
    </AppearanceContext.Provider>
  )
}

export function useAppearance(): AppearanceContextValue {
  const ctx = useContext(AppearanceContext)
  if (!ctx) {
    throw new Error('useAppearance must be used within AppearanceProvider')
  }
  return ctx
}

export { DEFAULT_APPEARANCE }
