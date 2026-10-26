import { useState, type FormEvent } from 'react'
import { ssoStartUrl } from '../api'
import { useAuth } from '../auth'
import { LanguageMenu } from '../components/LanguageMenu'
import { useI18n } from '../i18n'

export function Login() {
  const { status, login, bootstrap, register } = useAuth()
  const { t } = useI18n()
  const [mode, setMode] = useState<'login' | 'register'>('login')
  const [email, setEmail] = useState('admin@vulyk.local')
  const [password, setPassword] = useState('')
  const [displayName, setDisplayName] = useState('Admin')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const isBootstrap = Boolean(status?.bootstrap_available)
  const registrationMode = status?.registration_mode ?? 'disabled'
  const canRegister = !isBootstrap && registrationMode === 'open'
  const showRegister = mode === 'register' && canRegister
  const ssoProviders = (status?.sso_providers ?? []).filter((p) => p.enabled)

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      if (isBootstrap) {
        await bootstrap(email, password, displayName)
      } else if (showRegister) {
        await register(email, password, displayName)
      } else {
        await login(email, password)
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="login-shell">
      <div className="login-shell-actions">
        <LanguageMenu />
      </div>
      <form className="login-panel" onSubmit={(e) => void onSubmit(e)}>
        <div className="brand">
          <span className="brand-mark">
            {status?.instance_name || 'Vulyk'}
          </span>
          <span className="brand-tag">
            {isBootstrap
              ? t('login.bootstrapTitle')
              : showRegister
                ? t('login.registerTitle')
                : t('login.title')}
          </span>
        </div>
        <p className="muted">
          {isBootstrap
            ? t('login.bootstrapSubtitle')
            : t('login.subtitle')}
        </p>
        {(isBootstrap || showRegister) && (
          <label className="field">
            <span>{t('login.displayName')}</span>
            <input
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              required
              autoComplete="name"
            />
          </label>
        )}
        <label className="field">
          <span>{t('login.email')}</span>
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
            autoComplete="username"
          />
        </label>
        <label className="field">
          <span>{t('login.password')}</span>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
            minLength={8}
            autoComplete={
              isBootstrap || showRegister ? 'new-password' : 'current-password'
            }
          />
        </label>
        {error && <p className="error">{error}</p>}
        <button className="btn" type="submit" disabled={busy}>
          {busy
            ? t('login.working')
            : isBootstrap
              ? t('login.createAdmin')
              : showRegister
                ? t('login.registerSubmit')
                : t('login.submit')}
        </button>

        {!isBootstrap && canRegister && (
          <button
            type="button"
            className="btn secondary"
            style={{ marginTop: '0.5rem' }}
            onClick={() =>
              setMode((m) => (m === 'login' ? 'register' : 'login'))
            }
          >
            {showRegister ? t('login.backToSignIn') : t('login.register')}
          </button>
        )}

        {!isBootstrap && ssoProviders.length > 0 && (
          <div style={{ marginTop: '1rem' }}>
            {ssoProviders.map((p) => (
              <button
                key={p.id}
                type="button"
                className="btn secondary"
                style={{ width: '100%', marginTop: '0.35rem' }}
                onClick={() => {
                  window.location.href = ssoStartUrl(p.provider_key)
                }}
              >
                {t('login.sso', { name: p.display_name })}
              </button>
            ))}
          </div>
        )}
      </form>
    </div>
  )
}
