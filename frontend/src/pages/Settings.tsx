import { useEffect, useMemo, useState, type FormEvent } from 'react'
import { Link } from 'react-router-dom'
import {
  api,
  type ApiTokenPublic,
  type AutomationRepo,
  type AutomationRepoBranch,
  type Project,
  type ProjectCiToken,
  type ProjectRole,
} from '../api'
import { useAuth } from '../auth'
import { PasswordRequirements } from '../components/PasswordRequirements'
import { LOCALES, useI18n, type Locale } from '../i18n'
import {
  DEFAULT_PASSWORD_POLICY,
  passwordMeetsPolicy,
} from '../passwordPolicy'
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

type RepoFormState = {
  id: string | null
  name: string
  provider: string
  remote_url: string
  branch: string
  subpath: string
  token: string
  actions_token: string
  ci_workflow_file: string
  ci_workflow_ref: string
  poll_interval_minutes: number
  webhook_secret: string
  webhook_url: string
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
  actions_token: '',
  ci_workflow_file: '',
  ci_workflow_ref: 'main',
  poll_interval_minutes: 15,
  webhook_secret: '',
  webhook_url: '',
  project_ids: [],
})

function formatRateLimitMessage(
  t: (key: string, vars?: Record<string, string | number>) => string,
  resetAt: string | null | undefined,
): string {
  const local = resetAt
    ? new Date(resetAt).toLocaleString()
    : t('settings.automationRateLimitUnknownReset')
  return t('settings.automationRateLimit', { reset: local })
}

function formatRepoStatus(
  t: (key: string, vars?: Record<string, string | number>) => string,
  repo: AutomationRepo,
): string {
  const indexed = repo.last_indexed_at
    ? t('settings.automationLastIndexed', {
        time: new Date(repo.last_indexed_at).toLocaleString(),
      })
    : ''
  const sha = repo.commit_sha ? repo.commit_sha.slice(0, 8) : ''
  const trigger = repo.last_index_trigger
    ? t(`settings.automationTrigger.${repo.last_index_trigger}` as 'settings.automationTrigger.manual')
    : ''
  return [
    repo.index_status,
    t('settings.automationFilesChunks', {
      files: repo.file_count,
      chunks: repo.chunk_count,
    }),
    indexed,
    sha ? t('settings.automationSha', { sha }) : '',
    trigger ? t('settings.automationTriggerLabel', { trigger }) : '',
    repo.has_token ? t('settings.automationTokenSet') : '',
  ]
    .filter(Boolean)
    .join(' · ')
}

export function Settings() {
  const { user, status, refreshStatus, changePassword: changePasswordAuth } =
    useAuth()
  const { t, locale, setLocale } = useI18n()
  const policy = status?.password_policy ?? DEFAULT_PASSWORD_POLICY
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
  const [repoBranches, setRepoBranches] = useState<AutomationRepoBranch[]>([])
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

  const loadRepoBranches = async (form: RepoFormState = repoForm) => {
    if (!form.remote_url.trim()) return
    if (form.provider !== 'github' && form.provider !== 'gitlab') {
      setRepoBranches([])
      return
    }
    try {
      const res =
        form.id && !form.token
          ? await api.listAutomationRepoBranches(form.id)
          : await api.listAutomationRepoBranchesPreview({
              provider: form.provider,
              remote_url: form.remote_url.trim(),
              ...(form.token ? { token: form.token } : {}),
              project_ids: form.project_ids,
            })
      setRepoBranches(res.branches ?? [])
    } catch {
      /* keep prior list; branch can still be typed */
    }
  }

  const startEditRepo = (repo: AutomationRepo) => {
    const next = {
      id: repo.id,
      name: repo.name,
      provider: repo.provider,
      remote_url: repo.remote_url,
      branch: repo.branch,
      subpath: repo.subpath,
      token: '',
      actions_token: '',
      ci_workflow_file: repo.ci_workflow_file || '',
      ci_workflow_ref: repo.ci_workflow_ref || 'main',
      poll_interval_minutes: repo.poll_interval_minutes ?? 15,
      webhook_secret: repo.webhook_secret || '',
      webhook_url: repo.webhook_url || '',
      project_ids: repo.project_ids?.length
        ? [...repo.project_ids]
        : repo.project_id
          ? [repo.project_id]
          : [],
    }
    setRepoForm(next)
    setRepoInfo(formatRepoStatus(t, repo))
    void loadRepoBranches(next)
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
          ...(repoForm.actions_token
            ? { actions_token: repoForm.actions_token }
            : {}),
          ci_workflow_file: repoForm.ci_workflow_file.trim(),
          ci_workflow_ref: repoForm.ci_workflow_ref.trim() || 'main',
          poll_interval_minutes: repoForm.poll_interval_minutes,
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
          ...(repoForm.actions_token
            ? { actions_token: repoForm.actions_token }
            : {}),
          ci_workflow_file: repoForm.ci_workflow_file.trim(),
          ci_workflow_ref: repoForm.ci_workflow_ref.trim() || 'main',
          poll_interval_minutes: repoForm.poll_interval_minutes,
          project_ids: repoForm.project_ids,
        })
      }
      setRepoForm(emptyRepoForm())
      setRepoBranches([])
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
    if (!repoForm.remote_url.trim()) return
    setRepoBusy(true)
    setError(null)
    try {
      const res =
        repoForm.id && !repoForm.token
          ? await api.testAutomationRepoById(repoForm.id)
          : await api.testAutomationRepoPreview({
              provider: repoForm.provider,
              remote_url: repoForm.remote_url.trim(),
              branch: repoForm.branch.trim() || 'main',
              subpath: repoForm.subpath.trim(),
              ...(repoForm.token ? { token: repoForm.token } : {}),
              project_ids: repoForm.project_ids,
            })
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
      if (res.error_code === 'git_host_rate_limit') {
        setError(formatRateLimitMessage(t, res.rate_limit_reset_at))
        setRepoInfo(res.error)
      } else {
        setRepoInfo(
          res.error
            ? res.error
            : t('settings.automationIndexed', {
                files: res.file_count,
                chunks: res.chunk_count,
              }),
        )
      }
      await refreshRepos()
      const updated = (await api.listAutomationRepos()).find(
        (r) => r.id === repoForm.id,
      )
      if (updated) setRepoInfo(formatRepoStatus(t, updated))
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      if (/rate limit/i.test(msg)) {
        setError(formatRateLimitMessage(t, null))
      } else {
        setError(msg)
      }
    } finally {
      setRepoBusy(false)
    }
  }

  const onRotateWebhook = async () => {
    if (!repoForm.id) return
    setRepoBusy(true)
    setError(null)
    try {
      const updated = await api.updateAutomationRepo(repoForm.id, {
        rotate_webhook_secret: true,
      })
      setRepoForm((f) => ({
        ...f,
        webhook_secret: updated.webhook_secret,
        webhook_url: updated.webhook_url,
      }))
      setMessage(t('settings.automationWebhookRotated'))
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
            minLength={policy.min_length}
            autoComplete="new-password"
            {...testIdProps(TestIds.settingsNewPasswordInput)}
          />
          <PasswordRequirements password={newPassword} policy={policy} />
        </label>
        <label className="field">
          <span>{t('password.confirm')}</span>
          <input
            type="password"
            value={confirmPassword}
            onChange={(e) => setConfirmPassword(e.target.value)}
            required
            minLength={policy.min_length}
            autoComplete="new-password"
            {...testIdProps(TestIds.settingsConfirmPasswordInput)}
          />
        </label>
        <button
          className="btn"
          type="submit"
          disabled={busy || !passwordMeetsPolicy(newPassword, policy)}
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
        <section
          className="panel"
          {...testIdProps(TestIds.settingsAutomationPanel)}
        >
          <h3>{t('settings.automationRepos')}</h3>
          <p className="muted">
            {t('settings.automationReposHelp')}{' '}
            <a
              href={CI_GUIDE_URL}
              target="_blank"
              rel="noreferrer"
              {...testIdProps(TestIds.settingsAutomationGuideLink)}
            >
              {t('settings.automationReposGuide')}
            </a>
          </p>

          {repos.length === 0 ? (
            <p className="muted">{t('settings.automationEmpty')}</p>
          ) : (
            <ul className="plain-list">
              {repos.map((repo, index) => (
                <li
                  key={repo.id}
                  {...listItemProps(TestIds.settingsAutomationRepoRow, {
                    entityId: repo.id,
                    index,
                  })}
                >
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
                    {...listChildProps(
                      TestIds.settingsAutomationRepoRow,
                      'edit',
                    )}
                  >
                    {t('settings.automationEdit')}
                  </button>
                  <button
                    type="button"
                    className="linkish"
                    disabled={repoBusy}
                    onClick={() => void onDeleteRepo(repo.id)}
                    {...listChildProps(
                      TestIds.settingsAutomationRepoRow,
                      'delete',
                    )}
                  >
                    {t('settings.automationDelete')}
                  </button>
                </li>
              ))}
            </ul>
          )}

          {repoInfo && (
            <p
              className="notice"
              {...testIdProps(TestIds.settingsAutomationStatus)}
            >
              {repoInfo}
            </p>
          )}

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
                {...testIdProps(TestIds.settingsAutomationNameInput)}
              />
            </label>
            <label className="field">
              <span>{t('admin.automationProvider')}</span>
              <select
                value={repoForm.provider}
                onChange={(e) => {
                  setRepoForm((f) => ({ ...f, provider: e.target.value }))
                  setRepoBranches([])
                }}
                {...testIdProps(TestIds.settingsAutomationProviderSelect)}
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
                {...testIdProps(TestIds.settingsAutomationUrlInput)}
              />
            </label>
            <label className="field">
              <span>{t('admin.automationBranch')}</span>
              {repoForm.provider === 'github' ||
              repoForm.provider === 'gitlab' ? (
                <div className="form-row" style={{ gap: '0.5rem' }}>
                  <select
                    value={
                      repoBranches.some((b) => b.name === repoForm.branch)
                        ? repoForm.branch
                        : ''
                    }
                    onChange={(e) =>
                      setRepoForm((f) => ({ ...f, branch: e.target.value }))
                    }
                    {...testIdProps(TestIds.settingsAutomationBranchSelect)}
                  >
                    <option value="">
                      {t('settings.automationBranchCustom')}
                    </option>
                    {repoBranches.map((b) => (
                      <option key={b.name} value={b.name}>
                        {b.name}
                        {b.is_new ? ` (${t('settings.automationBranchNew')})` : ''}
                      </option>
                    ))}
                  </select>
                  <input
                    value={repoForm.branch}
                    onChange={(e) =>
                      setRepoForm((f) => ({ ...f, branch: e.target.value }))
                    }
                    required
                    {...testIdProps(TestIds.settingsAutomationBranchInput)}
                  />
                  <button
                    type="button"
                    className="btn secondary"
                    disabled={repoBusy || !repoForm.remote_url.trim()}
                    onClick={() => void loadRepoBranches()}
                    {...testIdProps(
                      TestIds.settingsAutomationRefreshBranchesButton,
                    )}
                  >
                    {t('settings.automationRefreshBranches')}
                  </button>
                </div>
              ) : (
                <input
                  value={repoForm.branch}
                  onChange={(e) =>
                    setRepoForm((f) => ({ ...f, branch: e.target.value }))
                  }
                  required
                  {...testIdProps(TestIds.settingsAutomationBranchInput)}
                />
              )}
            </label>
            <label className="field">
              <span>{t('admin.automationSubpath')}</span>
              <input
                value={repoForm.subpath}
                onChange={(e) =>
                  setRepoForm((f) => ({ ...f, subpath: e.target.value }))
                }
                placeholder="e2e"
                {...testIdProps(TestIds.settingsAutomationSubpathInput)}
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
                {...testIdProps(TestIds.settingsAutomationTokenInput)}
              />
            </label>
            <label className="field">
              <span>{t('admin.automationActionsToken')}</span>
              <input
                type="password"
                value={repoForm.actions_token}
                onChange={(e) =>
                  setRepoForm((f) => ({ ...f, actions_token: e.target.value }))
                }
                autoComplete="off"
                placeholder={t('admin.automationActionsTokenHint')}
                {...testIdProps(TestIds.settingsAutomationActionsTokenInput)}
              />
            </label>
            <label className="field">
              <span>{t('admin.automationWorkflowFile')}</span>
              <input
                value={repoForm.ci_workflow_file}
                onChange={(e) =>
                  setRepoForm((f) => ({
                    ...f,
                    ci_workflow_file: e.target.value,
                  }))
                }
                placeholder=".github/workflows/vulyk.yml"
                {...testIdProps(TestIds.settingsAutomationWorkflowFileInput)}
              />
            </label>
            <label className="field">
              <span>{t('admin.automationWorkflowRef')}</span>
              <input
                value={repoForm.ci_workflow_ref}
                onChange={(e) =>
                  setRepoForm((f) => ({
                    ...f,
                    ci_workflow_ref: e.target.value,
                  }))
                }
                placeholder="main"
                {...testIdProps(TestIds.settingsAutomationWorkflowRefInput)}
              />
            </label>
            <label className="field">
              <span>{t('settings.automationPollInterval')}</span>
              <input
                type="number"
                min={0}
                value={repoForm.poll_interval_minutes}
                onChange={(e) =>
                  setRepoForm((f) => ({
                    ...f,
                    poll_interval_minutes: Math.max(
                      0,
                      Number.parseInt(e.target.value || '0', 10) || 0,
                    ),
                  }))
                }
                {...testIdProps(TestIds.settingsAutomationPollIntervalInput)}
              />
              <span className="muted">
                {t('settings.automationPollIntervalHelp')}
              </span>
            </label>
            {repoForm.id ? (
              <div className="field">
                <span>{t('settings.automationWebhook')}</span>
                <p className="muted">{t('settings.automationWebhookHelp')}</p>
                <label className="field">
                  <span>{t('settings.automationWebhookUrl')}</span>
                  <input
                    readOnly
                    value={repoForm.webhook_url}
                    {...testIdProps(TestIds.settingsAutomationWebhookUrl)}
                  />
                </label>
                <label className="field">
                  <span>{t('settings.automationWebhookSecret')}</span>
                  <input
                    readOnly
                    value={repoForm.webhook_secret}
                    {...testIdProps(TestIds.settingsAutomationWebhookSecret)}
                  />
                </label>
                <button
                  type="button"
                  className="btn secondary"
                  disabled={repoBusy}
                  onClick={() => void onRotateWebhook()}
                  {...testIdProps(TestIds.settingsAutomationRotateWebhookButton)}
                >
                  {t('settings.automationRotateWebhook')}
                </button>
              </div>
            ) : null}
            <fieldset className="field">
              <legend>{t('settings.automationProjects')}</legend>
              {projectsForAutomation.length === 0 ? (
                <p className="muted">{t('admin.automationRepoNeedProject')}</p>
              ) : (
                projectsForAutomation.map((p, index) => (
                  <label key={p.id} className="check-row">
                    <input
                      type="checkbox"
                      checked={repoForm.project_ids.includes(p.id)}
                      onChange={() => toggleRepoProject(p.id)}
                      {...listItemProps(
                        TestIds.settingsAutomationProjectCheckbox,
                        {
                          entityId: p.id,
                          entityKey: p.key,
                          index,
                        },
                      )}
                    />
                    <span>
                      {p.key} — {p.name}
                    </span>
                  </label>
                ))
              )}
            </fieldset>
            <div className="form-row">
              <button
                className="btn"
                type="submit"
                disabled={repoBusy}
                {...testIdProps(TestIds.settingsAutomationSaveButton)}
              >
                {repoForm.id
                  ? t('admin.automationSave')
                  : t('settings.automationAdd')}
              </button>
              <button
                type="button"
                className="btn secondary"
                disabled={repoBusy || !repoForm.remote_url.trim()}
                onClick={() => void onTestRepo()}
                {...testIdProps(TestIds.settingsAutomationTestButton)}
              >
                {t('admin.automationTest')}
              </button>
              {repoForm.id ? (
                <>
                  <button
                    type="button"
                    className="btn secondary"
                    disabled={repoBusy}
                    onClick={() => void onReindexRepo()}
                    {...testIdProps(TestIds.settingsAutomationReindexButton)}
                  >
                    {t('admin.automationReindex')}
                  </button>
                  <button
                    type="button"
                    className="btn secondary"
                    disabled={repoBusy}
                    onClick={() => {
                      setRepoForm(emptyRepoForm())
                      setRepoBranches([])
                      setRepoInfo(null)
                    }}
                    {...testIdProps(TestIds.settingsAutomationCancelButton)}
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
        <section className="panel" {...testIdProps(TestIds.settingsCiPanel)}>
          <h3>{t('settings.ciTokens')}</h3>
          <p className="muted">
            {t('settings.ciTokensHelp')}{' '}
            <a
              href={CI_GUIDE_URL}
              target="_blank"
              rel="noreferrer"
              {...testIdProps(TestIds.settingsCiGuideLink)}
            >
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
              {...testIdProps(TestIds.settingsCiProjectSelect)}
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
            <p
              className="token-secret"
              {...testIdProps(TestIds.settingsCiTokenSecret)}
            >
              {t('settings.tokenCreated')} <code>{ciSecretOnce}</code>
            </p>
          )}
          {!ciProjectId ? null : ciTokens.length === 0 ? (
            <p className="muted">{t('settings.ciTokenEmpty')}</p>
          ) : (
            <ul className="plain-list">
              {ciTokens.map((tok, index) => (
                <li
                  key={tok.id}
                  {...listItemProps(TestIds.settingsCiTokenRow, {
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
                        void api
                          .revokeProjectCiToken(ciProjectId, tok.id)
                          .then(() => {
                            setCiSecretOnce(null)
                            void refreshCiTokens(ciProjectId)
                          })
                      }
                      {...listChildProps(
                        TestIds.settingsCiTokenRow,
                        'revoke',
                      )}
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
                {...testIdProps(TestIds.settingsCiTokenNameInput)}
              />
            </label>
            <button
              className="btn"
              type="submit"
              disabled={!ciProjectId}
              {...testIdProps(TestIds.settingsCiTokenCreateButton)}
            >
              {t('settings.ciTokenCreate')}
            </button>
          </form>
        </section>
      ) : null}
    </div>
  )
}
