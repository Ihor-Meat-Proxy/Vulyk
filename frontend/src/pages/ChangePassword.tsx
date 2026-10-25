import { useState, type FormEvent } from 'react'
import { useAuth } from '../auth'
import { useI18n } from '../i18n'

function passwordLooksStrong(password: string): boolean {
  if (password.length < 8) return false
  return /[a-zA-Z]/.test(password) && /\d/.test(password)
}

export function ChangePassword() {
  const { user, changePassword, logout } = useAuth()
  const { t } = useI18n()
  const [currentPassword, setCurrentPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [confirm, setConfirm] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const requireCurrent = Boolean(user && !user.must_change_password)

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault()
    if (newPassword !== confirm) {
      setError(t('password.mismatch'))
      return
    }
    setBusy(true)
    setError(null)
    try {
      await changePassword(
        newPassword,
        requireCurrent || currentPassword
          ? currentPassword || undefined
          : undefined,
      )
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="login-shell">
      <form className="login-panel" onSubmit={(e) => void onSubmit(e)}>
        <div className="brand">
          <span className="brand-mark">Vulyk</span>
          <span className="brand-tag">{t('password.title')}</span>
        </div>
        <p className="muted">{t('password.subtitle')}</p>
        {(requireCurrent || user?.must_change_password) && (
          <label className="field">
            <span>{t('password.current')}</span>
            <input
              type="password"
              value={currentPassword}
              onChange={(e) => setCurrentPassword(e.target.value)}
              autoComplete="current-password"
              required={requireCurrent}
            />
          </label>
        )}
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
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            required
            minLength={8}
            autoComplete="new-password"
          />
        </label>
        {error && <p className="error">{error}</p>}
        <button
          className="btn"
          type="submit"
          disabled={busy || !passwordLooksStrong(newPassword)}
        >
          {busy ? t('login.working') : t('password.submit')}
        </button>
        <button
          type="button"
          className="btn secondary"
          style={{ marginTop: '0.5rem' }}
          onClick={() => void logout()}
        >
          {t('nav.signOut')}
        </button>
      </form>
    </div>
  )
}
