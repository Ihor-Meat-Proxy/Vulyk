import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react'
import { api, type AuthStatus, type UserPublic } from './api'
import { getStoredToken, setStoredToken } from './token'

type AuthContextValue = {
  status: AuthStatus | null
  user: UserPublic | null
  token: string | null
  loading: boolean
  refreshStatus: () => Promise<void>
  login: (email: string, password: string) => Promise<void>
  bootstrap: (
    email: string,
    password: string,
    displayName: string,
  ) => Promise<void>
  logout: () => Promise<void>
  setToken: (token: string | null) => void
}

const AuthContext = createContext<AuthContextValue | null>(null)

export function AuthProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<AuthStatus | null>(null)
  const [token, setTokenState] = useState<string | null>(() => getStoredToken())
  const [loading, setLoading] = useState(true)

  const setToken = useCallback((next: string | null) => {
    setTokenState(next)
    setStoredToken(next)
  }, [])

  const refreshStatus = useCallback(async () => {
    const next = await api.authStatus()
    setStatus(next)
  }, [])

  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        await refreshStatus()
      } catch {
        if (!cancelled) {
          setStatus({
            auth_required: false,
            bootstrap_available: false,
            authenticated: false,
            user: null,
          })
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [refreshStatus, token])

  const login = useCallback(
    async (email: string, password: string) => {
      const res = await api.login({ email, password })
      setToken(res.token)
      await refreshStatus()
    },
    [refreshStatus, setToken],
  )

  const bootstrap = useCallback(
    async (email: string, password: string, displayName: string) => {
      const res = await api.bootstrap({
        email,
        password,
        display_name: displayName,
      })
      setToken(res.token)
      await refreshStatus()
    },
    [refreshStatus, setToken],
  )

  const logout = useCallback(async () => {
    try {
      await api.logout()
    } finally {
      setToken(null)
      await refreshStatus()
    }
  }, [refreshStatus, setToken])

  const value = useMemo<AuthContextValue>(
    () => ({
      status,
      user: status?.user ?? null,
      token,
      loading,
      refreshStatus,
      login,
      bootstrap,
      logout,
      setToken,
    }),
    [
      status,
      token,
      loading,
      refreshStatus,
      login,
      bootstrap,
      logout,
      setToken,
    ],
  )

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>
}

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext)
  if (!ctx) {
    throw new Error('useAuth must be used within AuthProvider')
  }
  return ctx
}
