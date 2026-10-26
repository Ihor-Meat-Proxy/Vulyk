import { useState, type FormEvent } from 'react'
import { api, type RegistrationMode } from '../api'
import { useAuth } from '../auth'
import { LOCALES, normalizeLocale, useI18n, type Locale } from '../i18n'
import { TestIds, testIdProps } from '../testing'
import { browserTimeZone } from '../timezone'

function passwordLooksStrong(password: string): boolean {
  if (password.length < 8) return false
  const hasLetter = /[a-zA-Z]/.test(password)
  const hasDigit = /\d/.test(password)
  return hasLetter && hasDigit
}

export function SetupWizard() {
  const { refreshStatus, status } = useAuth()
  const { t } = useI18n()
  const [setupToken, setSetupToken] = useState('')
  const [email, setEmail] = useState('admin@vulyk.local')
  const [password, setPassword] = useState('')
  const [displayName, setDisplayName] = useState('Admin')
  const [instanceName, setInstanceName] = useState(
    status?.instance_name || 'Vulyk',
  )
  const [baseUrl, setBaseUrl] = useState(
    typeof window !== 'undefined' ? window.location.origin : '',
  )
  const [locale, setLocale] = useState<Locale>(
    () => normalizeLocale(status?.default_locale) ?? 'en',
  )
  const [timezone, setTimezone] = useState(
    () => status?.default_timezone || browserTimeZone(),
  )
  const [registrationMode, setRegistrationMode] =
    useState<RegistrationMode>('disabled')
  const [seedSample, setSeedSample] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [done, setDone] = useState(false)
  const [busy, setBusy] = useState(false)

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      await api.completeSetup({
        setup_token: setupToken.trim(),
        email: email.trim(),
        password,
        display_name: displayName.trim(),
        instance_name: instanceName.trim() || undefined,
        base_url: baseUrl.trim() || undefined,
        default_locale: locale,
        default_timezone: timezone.trim() || undefined,
        registration_mode: registrationMode,
        seed_sample: seedSample,
      })
      await refreshStatus()
      setDone(true)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  if (done && !status?.setup_required) {
    return (
      <div className="login-shell">
        <div className="login-panel" {...testIdProps(TestIds.setupSuccess)}>
          <div className="brand">
            <span className="brand-mark">Vulyk</span>
            <span className="brand-tag">{t('setup.title')}</span>
          </div>
          <p>{t('setup.success')}</p>
        </div>
      </div>
    )
  }

  return (
    <div className="login-shell">
      <form
        className="login-panel"
        onSubmit={(e) => void onSubmit(e)}
        {...testIdProps(TestIds.setupPanel)}
      >
        <div className="brand">
          <span className="brand-mark">Vulyk</span>
          <span className="brand-tag">{t('setup.title')}</span>
        </div>
        <p className="muted">{t('setup.subtitle')}</p>

        <label className="field">
          <span>{t('setup.token')}</span>
          <input
            value={setupToken}
            onChange={(e) => setSetupToken(e.target.value)}
            required
            autoComplete="off"
            {...testIdProps(TestIds.setupTokenInput)}
          />
        </label>
        <label className="field">
          <span>{t('setup.displayName')}</span>
          <input
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
            required
            autoComplete="name"
            {...testIdProps(TestIds.setupDisplayNameInput)}
          />
        </label>
        <label className="field">
          <span>{t('setup.adminEmail')}</span>
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
            autoComplete="username"
            {...testIdProps(TestIds.setupEmailInput)}
          />
        </label>
        <label className="field">
          <span>{t('setup.adminPassword')}</span>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
            minLength={8}
            autoComplete="new-password"
            {...testIdProps(TestIds.setupPasswordInput)}
          />
          <span className="muted" style={{ fontSize: '0.85rem' }}>
            {t('setup.passwordHint')}
            {password && !passwordLooksStrong(password)
              ? ' — weak'
              : password
                ? ' — ok'
                : ''}
          </span>
        </label>
        <label className="field">
          <span>{t('setup.instanceName')}</span>
          <input
            value={instanceName}
            onChange={(e) => setInstanceName(e.target.value)}
            {...testIdProps(TestIds.setupInstanceNameInput)}
          />
        </label>
        <label className="field">
          <span>{t('setup.baseUrl')}</span>
          <input
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder="https://vulyk.example.com"
            {...testIdProps(TestIds.setupBaseUrlInput)}
          />
        </label>
        <label className="field">
          <span>{t('setup.locale')}</span>
          <select
            value={locale}
            aria-label={t('setup.locale')}
            onChange={(e) => setLocale(e.target.value as Locale)}
            {...testIdProps(TestIds.setupLocaleSelect)}
          >
            {LOCALES.map((meta) => (
              <option key={meta.code} value={meta.code}>
                {meta.nativeName}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>{t('setup.timezone')}</span>
          <input
            value={timezone}
            onChange={(e) => setTimezone(e.target.value)}
            placeholder="Europe/Kyiv"
            required
            {...testIdProps(TestIds.setupTimezoneInput)}
          />
        </label>
        <label className="field">
          <span>{t('setup.registrationMode')}</span>
          <select
            value={registrationMode}
            onChange={(e) =>
              setRegistrationMode(e.target.value as RegistrationMode)
            }
            {...testIdProps(TestIds.setupRegistrationSelect)}
          >
            <option value="open">{t('setup.modeOpen')}</option>
            <option value="invite">{t('setup.modeInvite')}</option>
            <option value="disabled">{t('setup.modeDisabled')}</option>
          </select>
        </label>
        <label className="field" style={{ flexDirection: 'row', gap: '0.5rem' }}>
          <input
            type="checkbox"
            checked={seedSample}
            onChange={(e) => setSeedSample(e.target.checked)}
            {...testIdProps(TestIds.setupSeedSampleCheckbox)}
          />
          <span>{t('setup.seedSample')}</span>
        </label>

        {error && (
          <p className="error" {...testIdProps(TestIds.setupError)}>
            {error}
          </p>
        )}
        <button
          className="btn"
          type="submit"
          disabled={busy || !passwordLooksStrong(password)}
          {...testIdProps(TestIds.setupSubmitButton)}
        >
          {busy ? t('login.working') : t('setup.submit')}
        </button>
      </form>
    </div>
  )
}
