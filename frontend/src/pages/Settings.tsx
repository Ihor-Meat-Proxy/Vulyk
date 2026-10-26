import { useEffect, useState, type FormEvent } from 'react'
import { Link } from 'react-router-dom'
import { api, type ApiTokenPublic } from '../api'
import { useAuth } from '../auth'
import { LOCALES, useI18n, type Locale } from '../i18n'
import { browserTimeZone } from '../timezone'

type NotifyPrefs = {
  notify_email: boolean
  notify_in_app: boolean
  notify_run_updates: boolean
}

function readNotify(prefs: {
  notify_email?: boolean
  notify_in_app?: boolean
  notify_run_updates?: boolean
} | null | undefined): NotifyPrefs {
  return {
    notify_email: prefs?.notify_email ?? true,
    notify_in_app: prefs?.notify_in_app ?? true,
    notify_run_updates: prefs?.notify_run_updates ?? true,
  }
}

function passwordLooksStrong(password: string): boolean {
  if (password.length < 8) return false
  return /[a-zA-Z]/.test(password) && /\d/.test(password)
}

export function Settings() {
  const { user, refreshStatus, changePassword: changePasswordAuth } = useAuth()
  const { t, locale, setLocale } = useI18n()
  const [displayName, setDisplayName] = useState(user?.display_name ?? '')
  const [timezone, setTimezone] = useState(
    user?.timezone || browserTimeZone(),
  )
  const [notify, setNotify] = useState<NotifyPrefs>(() =>
    readNotify(user?.preferences),
  )
  const [tokens, setTokens] = useState<ApiTokenPublic[]>([])
  const [tokenName, setTokenName] = useState('ci-mcp')
  const [secretOnce, setSecretOnce] = useState<string | null>(null)
  const [canTokens, setCanTokens] = useState(false)
  const [currentPassword, setCurrentPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [message, setMessage] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    setDisplayName(user?.display_name ?? '')
    setTimezone(user?.timezone || browserTimeZone())
    setNotify(readNotify(user?.preferences))
  }, [user])

  const refreshTokens = async () => {
    try {
      setTokens(await api.listTokens())
      setCanTokens(true)
    } catch {
      setCanTokens(false)
      setTokens([])
    }
  }

  useEffect(() => {
    if (!user) return
    void refreshTokens()
  }, [user?.id])

  if (!user) {
    return (
      <div className="panel">
        <h2>{t('settings.title')}</h2>
        <p className="muted">{t('nav.signIn')}</p>
        <Link className="btn" to="/login">
          {t('nav.signIn')}
        </Link>
      </div>
    )
  }

  const onSaveProfile = async (e: FormEvent) => {
    e.preventDefault()
    setBusy(true)
    setError(null)
    setMessage(null)
    try {
      await api.updateMe({
        display_name: displayName.trim() || user.display_name,
        locale,
        timezone: timezone.trim() || undefined,
        preferences: {
          theme: user.preferences.theme,
          font_size: user.preferences.font_size,
          density: user.preferences.density,
          reduced_motion: user.preferences.reduced_motion,
          notify_email: notify.notify_email,
          notify_in_app: notify.notify_in_app,
          notify_run_updates: notify.notify_run_updates,
        },
      })
      await refreshStatus()
      setMessage(t('settings.saved'))
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const onChangePassword = async (e: FormEvent) => {
    e.preventDefault()
    if (newPassword !== confirmPassword) {
      setError(t('password.mismatch'))
      return
    }
    setBusy(true)
    setError(null)
    setMessage(null)
    try {
      await changePasswordAuth(newPassword, currentPassword || undefined)
      setCurrentPassword('')
      setNewPassword('')
      setConfirmPassword('')
      setMessage(t('settings.saved'))
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const onCreateToken = async (e: FormEvent) => {
    e.preventDefault()
    setError(null)
    try {
      const res = await api.createToken(tokenName.trim() || 'token')
      setSecretOnce(res.secret)
      await refreshTokens()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  return (
    <div className="settings-page">
      <header>
        <h2>{t('settings.title')}</h2>
        <p className="muted">{t('settings.subtitle')}</p>
      </header>

      {message && <p className="notice">{message}</p>}
      {error && <p className="error">{error}</p>}

      <form className="panel stack-form" onSubmit={(e) => void onSaveProfile(e)}>
        <h3>{t('settings.profile')}</h3>
        <label className="field">
          <span>{t('login.displayName')}</span>
          <input
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
            required
          />
        </label>
        <label className="field">
          <span>{t('login.email')}</span>
          <input value={user.email} disabled readOnly />
          <span className="muted" style={{ fontSize: '0.85rem' }}>
            {t('settings.emailReadonly')}
          </span>
        </label>
        <label className="field">
          <span>{t('settings.language')}</span>
          <select
            value={locale}
            onChange={(e) => void setLocale(e.target.value as Locale)}
            aria-label={t('settings.language')}
          >
            {LOCALES.map((meta) => (
              <option key={meta.code} value={meta.code}>
                {meta.nativeName}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>{t('settings.timezone')}</span>
          <input
            value={timezone}
            onChange={(e) => setTimezone(e.target.value)}
            placeholder="Europe/Paris"
            required
          />
        </label>

        <h3>{t('settings.notifications')}</h3>
        <p className="muted">{t('settings.notificationsHint')}</p>
        <label className="check-row">
          <input
            type="checkbox"
            checked={notify.notify_email}
            onChange={(e) =>
              setNotify((n) => ({ ...n, notify_email: e.target.checked }))
            }
          />
          <span>{t('settings.notifyEmail')}</span>
        </label>
        <label className="check-row">
          <input
            type="checkbox"
            checked={notify.notify_in_app}
            onChange={(e) =>
              setNotify((n) => ({ ...n, notify_in_app: e.target.checked }))
            }
          />
          <span>{t('settings.notifyInApp')}</span>
        </label>
        <label className="check-row">
          <input
            type="checkbox"
            checked={notify.notify_run_updates}
            onChange={(e) =>
              setNotify((n) => ({
                ...n,
                notify_run_updates: e.target.checked,
              }))
            }
          />
          <span>{t('settings.notifyRunUpdates')}</span>
        </label>

        <button className="btn" type="submit" disabled={busy}>
          {busy ? t('login.working') : t('common.save')}
        </button>
      </form>

      <form
        className="panel stack-form"
        onSubmit={(e) => void onChangePassword(e)}
      >
        <h3>{t('settings.password')}</h3>
        <label className="field">
          <span>{t('password.current')}</span>
          <input
            type="password"
            value={currentPassword}
            onChange={(e) => setCurrentPassword(e.target.value)}
            autoComplete="current-password"
          />
        </label>
        <label className="field">
          <span>{t('password.new')}</span>
          <input
            type="password"
            value={newPassword}
            onChange={(e) => setNewPassword(e.target.value)}
            required
            minLength={8}
            autoComplete="new-password"
          />
          <span className="muted" style={{ fontSize: '0.85rem' }}>
            {t('setup.passwordHint')}
          </span>
        </label>
        <label className="field">
          <span>{t('password.confirm')}</span>
          <input
            type="password"
            value={confirmPassword}
            onChange={(e) => setConfirmPassword(e.target.value)}
            required
            minLength={8}
            autoComplete="new-password"
          />
        </label>
        <button
          className="btn"
          type="submit"
          disabled={busy || !passwordLooksStrong(newPassword)}
        >
          {t('settings.passwordLink')}
        </button>
      </form>

      <section className="panel">
        <h3>{t('settings.tokens')}</h3>
        <p className="muted">{t('settings.tokensHint')}</p>
        {!canTokens ? (
          <p className="muted">{t('settings.tokensForbidden')}</p>
        ) : (
          <>
            {secretOnce && (
              <p className="token-secret">
                {t('settings.tokenCreated')}{' '}
                <code>{secretOnce}</code>
              </p>
            )}
            {tokens.length === 0 ? (
              <p className="muted">{t('settings.noTokens')}</p>
            ) : (
              <ul className="plain-list">
                {tokens.map((tok) => (
                  <li key={tok.id}>
                    <strong>{tok.name}</strong> · {tok.token_prefix}…
                    {tok.revoked_at ? null : (
                      <button
                        type="button"
                        className="linkish"
                        onClick={() =>
                          void api.revokeToken(tok.id).then(() => {
                            setSecretOnce(null)
                            void refreshTokens()
                          })
                        }
                      >
                        {t('settings.revokeToken')}
                      </button>
                    )}
                  </li>
                ))}
              </ul>
            )}
            <form
              className="stack-form"
              style={{ marginTop: '1rem' }}
              onSubmit={(e) => void onCreateToken(e)}
            >
              <label className="field">
                <span>{t('settings.tokenName')}</span>
                <input
                  value={tokenName}
                  onChange={(e) => setTokenName(e.target.value)}
                  required
                />
              </label>
              <button className="btn" type="submit">
                {t('settings.createToken')}
              </button>
            </form>
          </>
        )}
      </section>
    </div>
  )
}
