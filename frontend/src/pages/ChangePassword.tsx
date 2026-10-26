import { useState, type FormEvent } from 'react'
import { useAuth } from '../auth'
import { PasswordRequirements } from '../components/PasswordRequirements'
import { useI18n } from '../i18n'
import {
  DEFAULT_PASSWORD_POLICY,
  passwordMeetsPolicy,
} from '../passwordPolicy'
import { TestIds, testIdProps } from '../testing'

export function ChangePassword() {
  const { user, status, changePassword, logout } = useAuth()
  const { t } = useI18n()
  const [currentPassword, setCurrentPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [confirm, setConfirm] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const policy = status?.password_policy ?? DEFAULT_PASSWORD_POLICY
  const requireCurrent = Boolean(user && !user.must_change_password)
  const forced = Boolean(user?.must_change_password)

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
    <div
      className="login-shell"
      {...testIdProps(TestIds.changePasswordShell)}
    >
      <form
        className="login-panel"
        onSubmit={(e) => void onSubmit(e)}
        {...testIdProps(TestIds.changePasswordPanel)}
      >
        <div className="brand">
          <span className="brand-mark">Vulyk</span>
          <span className="brand-tag">{t('password.title')}</span>
        </div>
        <p
          className="muted"
          {...testIdProps(TestIds.changePasswordSubtitle)}
        >
          {forced ? t('password.forcedSubtitle') : t('password.subtitle')}
        </p>
        {(requireCurrent || forced) && (
          <label className="field">
            <span>
              {t('password.current')}
              {forced ? ` (${t('password.currentOptional')})` : ''}
            </span>
            <input
              type="password"
              value={currentPassword}
              onChange={(e) => setCurrentPassword(e.target.value)}
              autoComplete="current-password"
              required={requireCurrent}
              {...testIdProps(TestIds.changePasswordCurrentInput)}
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
            minLength={policy.min_length}
            autoComplete="new-password"
            {...testIdProps(TestIds.changePasswordNewInput)}
          />
          <PasswordRequirements password={newPassword} policy={policy} />
        </label>
        <label className="field">
          <span>{t('password.confirm')}</span>
          <input
            type="password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            required
            minLength={policy.min_length}
            autoComplete="new-password"
            {...testIdProps(TestIds.changePasswordConfirmInput)}
          />
        </label>
        {error && (
          <p className="error" {...testIdProps(TestIds.changePasswordError)}>
            {error}
          </p>
        )}
        <button
          className="btn"
          type="submit"
          disabled={busy || !passwordMeetsPolicy(newPassword, policy)}
          {...testIdProps(TestIds.changePasswordSubmitButton)}
        >
          {busy ? t('login.working') : t('password.submit')}
        </button>
        <button
          type="button"
          className="btn secondary"
          style={{ marginTop: '0.5rem' }}
          onClick={() => void logout()}
          {...testIdProps(TestIds.changePasswordSignOutButton)}
        >
          {t('nav.signOut')}
        </button>
      </form>
    </div>
  )
}
