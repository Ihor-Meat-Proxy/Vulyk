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
  register: (
    email: string,
    password: string,
    displayName: string,
    inviteToken?: string,
  ) => Promise<void>
  bootstrap: (
    email: string,
    password: string,
    displayName: string,
  ) => Promise<void>
  changePassword: (
    newPassword: string,
    currentPassword?: string,
  ) => Promise<void>
  logout: () => Promise<void>
  setToken: (token: string | null) => void
}

const AuthContext = createContext<AuthContextValue | null>(null)

const emptyStatus = (): AuthStatus => ({
  auth_required: false,
  bootstrap_available: false,
  setup_required: false,
  registration_mode: 'disabled',
  default_locale: 'en',
  default_timezone: 'UTC',
  instance_name: 'Vulyk',
  authenticated: false,
  user: null,
  sso_providers: [],
})

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
          setStatus(emptyStatus())
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

  const register = useCallback(
    async (
      email: string,
      password: string,
      displayName: string,
      inviteToken?: string,
    ) => {
      const res = await api.register({
        email,
        password,
        display_name: displayName,
        invite_token: inviteToken,
      })
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

  const changePassword = useCallback(
    async (newPassword: string, currentPassword?: string) => {
      await api.changePassword({
        new_password: newPassword,
        current_password: currentPassword,
      })
      await refreshStatus()
    },
    [refreshStatus],
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
      register,
      bootstrap,
      changePassword,
      logout,
      setToken,
    }),
    [
      status,
      token,
      loading,
      refreshStatus,
      login,
      register,
      bootstrap,
      changePassword,
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
