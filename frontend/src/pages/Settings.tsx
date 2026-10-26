import { useEffect, useMemo, useState, type FormEvent } from 'react'
import { Link } from 'react-router-dom'
import {
  api,
  type ApiTokenPublic,
  type AutomationRepo,
  type Project,
  type ProjectCiToken,
  type ProjectRole,
} from '../api'
import { useAuth } from '../auth'
import { LOCALES, useI18n, type Locale } from '../i18n'
import { can } from '../permissions'
import { TestIds, listChildProps, listItemProps, testIdProps } from '../testing'
import { browserTimeZone } from '../timezone'

const CI_GUIDE_URL =
  'https://github.com/Ihor-Meat-Proxy/Vulyk/blob/main/docs/guides/ci-integration.md'

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

type RepoFormState = {
  id: string | null
  name: string
  provider: string
  remote_url: string
  branch: string
  subpath: string
  token: string
  project_ids: string[]
}

const emptyRepoForm = (): RepoFormState => ({
  id: null,
  name: '',
  provider: 'github',
  remote_url: '',
  branch: 'main',
  subpath: '',
  token: '',
  project_ids: [],
})

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

  const [projects, setProjects] = useState<Project[]>([])
  const [projectRoles, setProjectRoles] = useState<Record<string, ProjectRole>>(
    {},
  )
  const [repos, setRepos] = useState<AutomationRepo[]>([])
  const [repoForm, setRepoForm] = useState<RepoFormState>(emptyRepoForm)
  const [repoBusy, setRepoBusy] = useState(false)
  const [repoInfo, setRepoInfo] = useState<string | null>(null)
  const [canAutomation, setCanAutomation] = useState(false)

  const [ciProjectId, setCiProjectId] = useState('')
  const [ciTokens, setCiTokens] = useState<ProjectCiToken[]>([])
  const [ciTokenName, setCiTokenName] = useState('github-main')
  const [ciSecretOnce, setCiSecretOnce] = useState<string | null>(null)
  const [canCiTokens, setCanCiTokens] = useState(false)

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

  const refreshRepos = async () => {
    try {
      setRepos(await api.listAutomationRepos())
    } catch {
      setRepos([])
    }
  }

  const refreshCiTokens = async (projectId: string) => {
    if (!projectId) {
      setCiTokens([])
      return
    }
    try {
      setCiTokens(await api.listProjectCiTokens(projectId))
    } catch {
      setCiTokens([])
    }
  }

  useEffect(() => {
    if (!user) return
    void refreshTokens()
    let cancelled = false
    void (async () => {
      try {
        const list = await api.listProjects()
        if (cancelled) return
        setProjects(list)
        const roles: Record<string, ProjectRole> = {}
        await Promise.all(
          list.map(async (p) => {
            try {
              const rows = await api.listMemberships(p.id)
              const mine = rows.find((m) => m.user_id === user.id)
              if (mine) roles[p.id] = mine.role
            } catch {
              /* no access */
            }
          }),
        )
        if (cancelled) return
        setProjectRoles(roles)
        const isAdmin = Boolean(user.is_system_admin)
        const anyAutomation = Object.values(roles).some((role) =>
          can({ role, isSystemAdmin: isAdmin }, 'manage_automation_repo'),
        )
        const anyCi = Object.values(roles).some((role) =>
          can({ role, isSystemAdmin: isAdmin }, 'manage_api_tokens'),
        )
        setCanAutomation(isAdmin || anyAutomation)
        setCanCiTokens(isAdmin || anyCi)
        if (isAdmin || anyAutomation) {
          await refreshRepos()
        }
        const firstCi = list.find((p) =>
          can(
            { role: roles[p.id] ?? null, isSystemAdmin: isAdmin },
            'manage_api_tokens',
          ),
        )
        if (firstCi) {
          setCiProjectId((cur) => cur || firstCi.id)
        }
      } catch {
        if (!cancelled) {
          setProjects([])
          setCanAutomation(Boolean(user.is_system_admin))
          setCanCiTokens(Boolean(user.is_system_admin))
        }
      }
    })()
    return () => {
      cancelled = true
    }
  }, [user?.id])

  useEffect(() => {
    if (!ciProjectId || !canCiTokens) return
    void refreshCiTokens(ciProjectId)
  }, [ciProjectId, canCiTokens])

  const projectsForCi = useMemo(
    () =>
      projects.filter((p) =>
        can(
          {
            role: projectRoles[p.id] ?? null,
            isSystemAdmin: Boolean(user?.is_system_admin),
          },
          'manage_api_tokens',
        ),
      ),
    [projects, projectRoles, user?.is_system_admin],
  )

  const projectsForAutomation = useMemo(
    () =>
      projects.filter((p) =>
        can(
          {
            role: projectRoles[p.id] ?? null,
            isSystemAdmin: Boolean(user?.is_system_admin),
          },
          'manage_automation_repo',
        ),
      ),
    [projects, projectRoles, user?.is_system_admin],
  )

  if (!user) {
    return (
      <div className="panel">
        <h2>{t('settings.title')}</h2>
        <p className="muted">{t('nav.signIn')}</p>
        <Link
          className="btn"
          to="/login"
          {...testIdProps(TestIds.settingsSignInLink)}
        >
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

  const startEditRepo = (repo: AutomationRepo) => {
    setRepoForm({
      id: repo.id,
      name: repo.name,
      provider: repo.provider,
      remote_url: repo.remote_url,
      branch: repo.branch,
      subpath: repo.subpath,
      token: '',
      project_ids: repo.project_ids?.length
        ? [...repo.project_ids]
        : repo.project_id
          ? [repo.project_id]
          : [],
    })
    setRepoInfo(
      `${repo.index_status} · files ${repo.file_count} · chunks ${repo.chunk_count}` +
        (repo.commit_sha ? ` · ${repo.commit_sha.slice(0, 8)}` : '') +
        (repo.has_token ? ' · token set' : ''),
    )
  }

  const toggleRepoProject = (projectId: string) => {
    setRepoForm((f) => ({
      ...f,
      project_ids: f.project_ids.includes(projectId)
        ? f.project_ids.filter((id) => id !== projectId)
        : [...f.project_ids, projectId],
    }))
  }

  const onSaveRepo = async (e: FormEvent) => {
    e.preventDefault()
    setRepoBusy(true)
    setError(null)
    setMessage(null)
    try {
      if (repoForm.id) {
        await api.updateAutomationRepo(repoForm.id, {
          name: repoForm.name.trim(),
          provider: repoForm.provider,
          remote_url: repoForm.remote_url.trim(),
          branch: repoForm.branch.trim() || 'main',
          subpath: repoForm.subpath.trim(),
          ...(repoForm.token ? { token: repoForm.token } : {}),
          project_ids: repoForm.project_ids,
        })
      } else {
        await api.createAutomationRepo({
          name: repoForm.name.trim(),
          provider: repoForm.provider,
          remote_url: repoForm.remote_url.trim(),
          branch: repoForm.branch.trim() || 'main',
          subpath: repoForm.subpath.trim(),
          ...(repoForm.token ? { token: repoForm.token } : {}),
          project_ids: repoForm.project_ids,
        })
      }
      setRepoForm(emptyRepoForm())
      setRepoInfo(null)
      setMessage(t('settings.automationSaved'))
      await refreshRepos()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setRepoBusy(false)
    }
  }

  const onDeleteRepo = async (id: string) => {
    if (!window.confirm(t('settings.automationDelete') + '?')) return
    setRepoBusy(true)
    setError(null)
    try {
      await api.deleteAutomationRepoById(id)
      if (repoForm.id === id) {
        setRepoForm(emptyRepoForm())
        setRepoInfo(null)
      }
      await refreshRepos()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setRepoBusy(false)
    }
  }

  const onTestRepo = async () => {
    if (!repoForm.id) return
    setRepoBusy(true)
    setError(null)
    try {
      const res = await api.testAutomationRepoById(repoForm.id)
      setRepoInfo(res.message)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setRepoBusy(false)
    }
  }

  const onReindexRepo = async () => {
    if (!repoForm.id) return
    setRepoBusy(true)
    setError(null)
    try {
      const res = await api.reindexAutomationRepoById(repoForm.id)
      setRepoInfo(
        res.error
          ? res.error
          : `Indexed ${res.file_count} files / ${res.chunk_count} chunks`,
      )
      await refreshRepos()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setRepoBusy(false)
    }
  }

  const onCreateCiToken = async (e: FormEvent) => {
    e.preventDefault()
    if (!ciProjectId) return
    setError(null)
    try {
      const res = await api.createProjectCiToken(
        ciProjectId,
        ciTokenName.trim() || 'ci-token',
      )
      setCiSecretOnce(res.secret)
      await refreshCiTokens(ciProjectId)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  const projectLabel = (id: string) => {
    const p = projects.find((x) => x.id === id)
    return p ? `${p.key} — ${p.name}` : id.slice(0, 8)
  }

  return (
    <div className="settings-page" {...testIdProps(TestIds.settingsPage)}>
      <header>
        <h2>{t('settings.title')}</h2>
        <p className="muted">{t('settings.subtitle')}</p>
      </header>

      {message && (
        <p className="notice" {...testIdProps(TestIds.settingsMessage)}>
          {message}
        </p>
      )}
      {error && (
        <p className="error" {...testIdProps(TestIds.settingsError)}>
          {error}
        </p>
      )}

      <form className="panel stack-form" onSubmit={(e) => void onSaveProfile(e)}>
        <h3>{t('settings.profile')}</h3>
        <label className="field">
          <span>{t('login.displayName')}</span>
          <input
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
            required
            {...testIdProps(TestIds.settingsDisplayNameInput)}
          />
        </label>
        <label className="field">
          <span>{t('login.email')}</span>
          <input
            value={user.email}
            disabled
            readOnly
            {...testIdProps(TestIds.settingsEmailInput)}
          />
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
            {...testIdProps(TestIds.settingsLocaleSelect)}
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
            {...testIdProps(TestIds.settingsTimezoneInput)}
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
            {...testIdProps(TestIds.settingsNotifyEmailCheckbox)}
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
            {...testIdProps(TestIds.settingsNotifyInAppCheckbox)}
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
            {...testIdProps(TestIds.settingsNotifyRunUpdatesCheckbox)}
          />
          <span>{t('settings.notifyRunUpdates')}</span>
        </label>

        <button
          className="btn"
          type="submit"
          disabled={busy}
          {...testIdProps(TestIds.settingsSaveProfileButton)}
        >
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
            {...testIdProps(TestIds.settingsCurrentPasswordInput)}
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
            {...testIdProps(TestIds.settingsNewPasswordInput)}
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
            {...testIdProps(TestIds.settingsConfirmPasswordInput)}
          />
        </label>
        <button
          className="btn"
          type="submit"
          disabled={busy || !passwordLooksStrong(newPassword)}
          {...testIdProps(TestIds.settingsChangePasswordButton)}
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
              <p
                className="token-secret"
                {...testIdProps(TestIds.settingsTokenSecret)}
              >
                {t('settings.tokenCreated')}{' '}
                <code>{secretOnce}</code>
              </p>
            )}
            {tokens.length === 0 ? (
              <p className="muted">{t('settings.noTokens')}</p>
            ) : (
              <ul
                className="plain-list"
                {...testIdProps(TestIds.settingsTokenList)}
              >
                {tokens.map((tok, index) => (
                  <li
                    key={tok.id}
                    {...listItemProps(TestIds.settingsTokenRow, {
                      // UUID only — token names are freeform and must not land in data-*
                      entityId: tok.id,
                      index,
                    })}
                  >
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
                        {...listChildProps(TestIds.settingsTokenRow, 'revoke')}
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
                  {...testIdProps(TestIds.settingsTokenNameInput)}
                />
              </label>
              <button
                className="btn"
                type="submit"
                {...testIdProps(TestIds.settingsCreateTokenButton)}
              >
                {t('settings.createToken')}
              </button>
            </form>
          </>
        )}
      </section>

      {canAutomation ? (
        <section className="panel">
          <h3>{t('settings.automationRepos')}</h3>
          <p className="muted">
            {t('settings.automationReposHelp')}{' '}
            <a href={CI_GUIDE_URL} target="_blank" rel="noreferrer">
              {t('settings.automationReposGuide')}
            </a>
          </p>

          {repos.length === 0 ? (
            <p className="muted">{t('settings.automationEmpty')}</p>
          ) : (
            <ul className="plain-list">
              {repos.map((repo) => (
                <li key={repo.id}>
                  <strong>{repo.name || repo.remote_url}</strong>
                  <span className="muted">
                    {' '}
                    · {repo.provider} · {repo.branch}
                    {repo.has_token ? ' · ••••' : ''}
                    {repo.project_ids?.length
                      ? ` · ${repo.project_ids.map(projectLabel).join(', ')}`
                      : ''}
                  </span>
                  <button
                    type="button"
                    className="linkish"
                    onClick={() => startEditRepo(repo)}
                  >
                    {t('settings.automationEdit')}
                  </button>
                  <button
                    type="button"
                    className="linkish"
                    disabled={repoBusy}
                    onClick={() => void onDeleteRepo(repo.id)}
                  >
                    {t('settings.automationDelete')}
                  </button>
                </li>
              ))}
            </ul>
          )}

          {repoInfo && <p className="notice">{repoInfo}</p>}

          <form
            className="stack-form"
            style={{ marginTop: '1rem' }}
            onSubmit={(e) => void onSaveRepo(e)}
          >
            <h4>
              {repoForm.id
                ? t('settings.automationEdit')
                : t('settings.automationAdd')}
            </h4>
            <label className="field">
              <span>{t('settings.automationName')}</span>
              <input
                value={repoForm.name}
                onChange={(e) =>
                  setRepoForm((f) => ({ ...f, name: e.target.value }))
                }
                required
              />
            </label>
            <label className="field">
              <span>{t('admin.automationProvider')}</span>
              <select
                value={repoForm.provider}
                onChange={(e) =>
                  setRepoForm((f) => ({ ...f, provider: e.target.value }))
                }
              >
                <option value="github">GitHub</option>
                <option value="gitlab">GitLab</option>
                <option value="generic">Generic HTTPS Git</option>
              </select>
            </label>
            <label className="field">
              <span>{t('admin.automationUrl')}</span>
              <input
                value={repoForm.remote_url}
                onChange={(e) =>
                  setRepoForm((f) => ({ ...f, remote_url: e.target.value }))
                }
                placeholder="https://github.com/org/repo.git"
                required
              />
            </label>
            <label className="field">
              <span>{t('admin.automationBranch')}</span>
              <input
                value={repoForm.branch}
                onChange={(e) =>
                  setRepoForm((f) => ({ ...f, branch: e.target.value }))
                }
                required
              />
            </label>
            <label className="field">
              <span>{t('admin.automationSubpath')}</span>
              <input
                value={repoForm.subpath}
                onChange={(e) =>
                  setRepoForm((f) => ({ ...f, subpath: e.target.value }))
                }
                placeholder="e2e"
              />
            </label>
            <label className="field">
              <span>{t('admin.automationToken')}</span>
              <input
                type="password"
                value={repoForm.token}
                onChange={(e) =>
                  setRepoForm((f) => ({ ...f, token: e.target.value }))
                }
                autoComplete="off"
                placeholder={t('admin.automationTokenHint')}
              />
            </label>
            <fieldset className="field">
              <legend>{t('settings.automationProjects')}</legend>
              {projectsForAutomation.length === 0 ? (
                <p className="muted">{t('admin.automationRepoNeedProject')}</p>
              ) : (
                projectsForAutomation.map((p) => (
                  <label key={p.id} className="check-row">
                    <input
                      type="checkbox"
                      checked={repoForm.project_ids.includes(p.id)}
                      onChange={() => toggleRepoProject(p.id)}
                    />
                    <span>
                      {p.key} — {p.name}
                    </span>
                  </label>
                ))
              )}
            </fieldset>
            <div className="form-row">
              <button className="btn" type="submit" disabled={repoBusy}>
                {repoForm.id
                  ? t('admin.automationSave')
                  : t('settings.automationAdd')}
              </button>
              {repoForm.id ? (
                <>
                  <button
                    type="button"
                    className="btn secondary"
                    disabled={repoBusy}
                    onClick={() => void onTestRepo()}
                  >
                    {t('admin.automationTest')}
                  </button>
                  <button
                    type="button"
                    className="btn secondary"
                    disabled={repoBusy}
                    onClick={() => void onReindexRepo()}
                  >
                    {t('admin.automationReindex')}
                  </button>
                  <button
                    type="button"
                    className="btn secondary"
                    disabled={repoBusy}
                    onClick={() => {
                      setRepoForm(emptyRepoForm())
                      setRepoInfo(null)
                    }}
                  >
                    {t('common.cancel')}
                  </button>
                </>
              ) : null}
            </div>
          </form>
        </section>
      ) : null}

      {canCiTokens ? (
        <section className="panel">
          <h3>{t('settings.ciTokens')}</h3>
          <p className="muted">
            {t('settings.ciTokensHelp')}{' '}
            <a href={CI_GUIDE_URL} target="_blank" rel="noreferrer">
              {t('settings.automationReposGuide')}
            </a>
          </p>
          <label className="field">
            <span>{t('settings.ciTokensProject')}</span>
            <select
              value={ciProjectId}
              onChange={(e) => {
                setCiProjectId(e.target.value)
                setCiSecretOnce(null)
              }}
              aria-label={t('settings.ciTokensProject')}
            >
              {projectsForCi.length === 0 && (
                <option value="">{t('admin.automationRepoNeedProject')}</option>
              )}
              {projectsForCi.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.key} — {p.name}
                </option>
              ))}
            </select>
          </label>
          {ciSecretOnce && (
            <p className="token-secret">
              {t('settings.tokenCreated')} <code>{ciSecretOnce}</code>
            </p>
          )}
          {!ciProjectId ? null : ciTokens.length === 0 ? (
            <p className="muted">{t('settings.ciTokenEmpty')}</p>
          ) : (
            <ul className="plain-list">
              {ciTokens.map((tok) => (
                <li key={tok.id}>
                  <strong>{tok.name}</strong> · {tok.token_prefix}…
                  {tok.revoked_at ? null : (
                    <button
                      type="button"
                      className="linkish"
                      onClick={() =>
                        void api
                          .revokeProjectCiToken(ciProjectId, tok.id)
                          .then(() => {
                            setCiSecretOnce(null)
                            void refreshCiTokens(ciProjectId)
                          })
                      }
                    >
                      {t('settings.ciTokenRevoke')}
                    </button>
                  )}
                </li>
              ))}
            </ul>
          )}
          <form
            className="stack-form"
            style={{ marginTop: '1rem' }}
            onSubmit={(e) => void onCreateCiToken(e)}
          >
            <label className="field">
              <span>{t('settings.tokenName')}</span>
              <input
                value={ciTokenName}
                onChange={(e) => setCiTokenName(e.target.value)}
                required
                disabled={!ciProjectId}
              />
            </label>
            <button className="btn" type="submit" disabled={!ciProjectId}>
              {t('settings.ciTokenCreate')}
            </button>
          </form>
        </section>
      ) : null}
    </div>
  )
}
