import { useCallback, useEffect, useMemo, useState, type FormEvent } from 'react'
import {
  api,
  type ApiTokenPublic,
  type InstanceSettings,
  type IntegrationSetting,
  type LlmConfig,
  type MembershipView,
  type Project,
  type ProjectRole,
  type ProviderKindInfo,
  type RegistrationMode,
  type SsoProviderAdmin,
  type UserPublic,
} from '../api'
import { useAuth } from '../auth'
import { useProjectPermissions } from '../hooks/useProjectPermissions'
import { useI18n } from '../i18n'
import { PROJECT_ROLES, roleDisplayName } from '../permissions'

type Tab =
  | 'users'
  | 'roles'
  | 'projects'
  | 'integrations'
  | 'llm'
  | 'tokens'
  | 'settings'
  | 'sso'

type Props = {
  projects: Project[]
  project: Project | null
  onProjectsChanged: () => void
}

export function Admin({ projects, project, onProjectsChanged }: Props) {
  const { user } = useAuth()
  const { t } = useI18n()
  const { can } = useProjectPermissions(project?.id ?? null)
  const [tab, setTab] = useState<Tab>('roles')
  const [error, setError] = useState<string | null>(null)
  const [users, setUsers] = useState<UserPublic[]>([])
  const [memberships, setMemberships] = useState<MembershipView[]>([])
  const [integrations, setIntegrations] = useState<IntegrationSetting[]>([])
  const [providers, setProviders] = useState<ProviderKindInfo[]>([])
  const [configs, setConfigs] = useState<LlmConfig[]>([])
  const [tokens, setTokens] = useState<ApiTokenPublic[]>([])
  const [secretOnce, setSecretOnce] = useState<string | null>(null)
  const [settings, setSettings] = useState<InstanceSettings | null>(null)
  const [ssoProviders, setSsoProviders] = useState<SsoProviderAdmin[]>([])

  const isSystemAdmin = Boolean(user?.is_system_admin)
  const canTokens = can('manage_api_tokens') || isSystemAdmin
  const canLlm = can('manage_llm_config') || isSystemAdmin
  const canAutomationRepo = can('manage_automation_repo') || isSystemAdmin
  const canRoles = can('manage_memberships') || isSystemAdmin
  const projectId = project?.id ?? null

  const tabs = useMemo(() => {
    const list: { id: Tab; label: string }[] = []
    if (isSystemAdmin) {
      list.push({ id: 'users', label: t('admin.users') })
      list.push({ id: 'settings', label: t('admin.settings') })
      list.push({ id: 'sso', label: t('admin.sso') })
    }
    if (isSystemAdmin || canAutomationRepo) {
      list.push({ id: 'integrations', label: t('admin.integrations') })
    }
    if (canRoles || isSystemAdmin) {
      list.push({ id: 'roles', label: t('admin.roles') })
    }
    list.push({ id: 'projects', label: t('admin.projects') })
    if (canLlm) list.push({ id: 'llm', label: t('admin.llm') })
    if (canTokens) list.push({ id: 'tokens', label: t('admin.tokens') })
    return list
  }, [isSystemAdmin, canRoles, canLlm, canTokens, canAutomationRepo, t])

  useEffect(() => {
    if (tabs.length > 0 && !tabs.some((x) => x.id === tab)) {
      setTab(tabs[0].id)
    }
  }, [tabs, tab])

  const load = useCallback(async () => {
    setError(null)
    try {
      if (isSystemAdmin) {
        setUsers(await api.listUsers())
        setIntegrations(await api.listAdminIntegrations())
        setSettings(await api.getAdminSettings())
        setSsoProviders(await api.listSsoProvidersAdmin())
      }
      if (projectId) {
        setMemberships(await api.listMemberships(projectId))
        if (canLlm) {
          setProviders(await api.listLlmProviders())
          setConfigs(await api.listLlmConfigs(projectId))
        }
      } else {
        setMemberships([])
        setConfigs([])
      }
      if (canTokens) {
        setTokens(await api.listTokens())
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [isSystemAdmin, projectId, canLlm, canTokens])

  useEffect(() => {
    void load()
  }, [load])

  return (
    <div className="admin">
      <div className="panel">
        <h2>{t('admin.title')}</h2>
        <p className="muted">{t('admin.subtitle')}</p>
        <div className="admin-tabs" role="tablist">
          {tabs.map(({ id, label }) => (
            <button
              key={id}
              type="button"
              role="tab"
              className={tab === id ? 'active' : undefined}
              aria-selected={tab === id}
              onClick={() => setTab(id)}
            >
              {label}
            </button>
          ))}
        </div>
        {error && <p className="error">{error}</p>}
      </div>

      {tab === 'users' && (
        <UsersPanel
          users={users}
          isAdmin={isSystemAdmin}
          onCreated={load}
          onError={setError}
        />
      )}
      {tab === 'roles' && (
        <RolesPanel
          project={project}
          users={users}
          memberships={memberships}
          isAdmin={isSystemAdmin}
          canManage={canRoles}
          onChanged={load}
          onError={setError}
        />
      )}
      {tab === 'projects' && (
        <ProjectsPanel
          projects={projects}
          onCreated={onProjectsChanged}
          onError={setError}
        />
      )}
      {tab === 'integrations' && (
        <>
          <IntegrationsPanel
            items={integrations}
            isAdmin={isSystemAdmin}
            onChanged={load}
            onError={setError}
          />
          {canAutomationRepo && (
            <AutomationRepoPanel
              projectId={projectId}
              onError={setError}
            />
          )}
        </>
      )}
      {tab === 'llm' && canLlm && (
        <LlmPanel
          projectId={projectId}
          providers={providers}
          configs={configs}
          onChanged={load}
          onError={setError}
        />
      )}
      {tab === 'tokens' && canTokens && (
        <TokensPanel
          tokens={tokens}
          secretOnce={secretOnce}
          onSecret={setSecretOnce}
          onChanged={load}
          onError={setError}
        />
      )}
      {tab === 'settings' && isSystemAdmin && (
        <SettingsPanel
          settings={settings}
          onChanged={load}
          onError={setError}
        />
      )}
      {tab === 'sso' && isSystemAdmin && (
        <SsoPanel
          providers={ssoProviders}
          onChanged={load}
          onError={setError}
        />
      )}
    </div>
  )
}

function UsersPanel({
  users,
  isAdmin,
  onCreated,
  onError,
}: {
  users: UserPublic[]
  isAdmin: boolean
  onCreated: () => void
  onError: (msg: string) => void
}) {
  const [email, setEmail] = useState('')
  const [displayName, setDisplayName] = useState('')
  const [password, setPassword] = useState('')
  const [systemAdmin, setSystemAdmin] = useState(false)

  if (!isAdmin) {
    return (
      <div className="panel">
        <p className="muted">
          System administrator access is required to manage users.
        </p>
      </div>
    )
  }

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault()
    try {
      await api.createUser({
        email,
        display_name: displayName,
        password,
        is_system_admin: systemAdmin,
      })
      setEmail('')
      setDisplayName('')
      setPassword('')
      setSystemAdmin(false)
      onCreated()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    }
  }

  return (
    <div className="panel admin-grid">
      <div>
        <h3>Users</h3>
        <table className="table">
          <thead>
            <tr>
              <th>Email</th>
              <th>Name</th>
              <th>Admin</th>
            </tr>
          </thead>
          <tbody>
            {users.map((u) => (
              <tr key={u.id}>
                <td>{u.email}</td>
                <td>{u.display_name}</td>
                <td>{u.is_system_admin ? 'yes' : '—'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <form className="stack-form" onSubmit={(e) => void onSubmit(e)}>
        <h3>Add user</h3>
        <label className="field">
          <span>Email</span>
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
          />
        </label>
        <label className="field">
          <span>Display name</span>
          <input
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
            required
          />
        </label>
        <label className="field">
          <span>Password</span>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            minLength={8}
            required
          />
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={systemAdmin}
            onChange={(e) => setSystemAdmin(e.target.checked)}
          />
          System admin
        </label>
        <button className="btn" type="submit">
          Create user
        </button>
      </form>
    </div>
  )
}

function RolesPanel({
  project,
  users,
  memberships,
  isAdmin,
  canManage,
  onChanged,
  onError,
}: {
  project: Project | null
  users: UserPublic[]
  memberships: MembershipView[]
  isAdmin: boolean
  canManage: boolean
  onChanged: () => void
  onError: (msg: string) => void
}) {
  const [userId, setUserId] = useState('')
  const [role, setRole] = useState<ProjectRole>('viewer')

  if (!project) {
    return (
      <div className="panel">
        <p className="muted">Select a project to manage memberships.</p>
      </div>
    )
  }

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault()
    if (!userId || !canManage) return
    try {
      await api.upsertMembership(project.id, { user_id: userId, role })
      onChanged()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    }
  }

  return (
    <div className="panel admin-grid">
      <div>
        <h3>Roles — {project.key}</h3>
        <p className="muted">
          Per-project: Admin, Test Architect, Test Automation Engineer,
          Developer, Viewer.
        </p>
        <table className="table">
          <thead>
            <tr>
              <th>User</th>
              <th>Role</th>
            </tr>
          </thead>
          <tbody>
            {memberships.map((m) => (
              <tr key={m.id}>
                <td>
                  {m.display_name} <span className="muted">({m.email})</span>
                </td>
                <td>
                  <span className="badge">{roleDisplayName(m.role)}</span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {canManage && (
        <form className="stack-form" onSubmit={(e) => void onSubmit(e)}>
          <h3>Assign role</h3>
          {!isAdmin && users.length === 0 && (
            <p className="muted">User directory requires system admin.</p>
          )}
          {users.length > 0 ? (
            <label className="field">
              <span>User</span>
              <select
                value={userId}
                onChange={(e) => setUserId(e.target.value)}
                required
              >
                <option value="">Select…</option>
                {users.map((u) => (
                  <option key={u.id} value={u.id}>
                    {u.email}
                  </option>
                ))}
              </select>
            </label>
          ) : (
            <label className="field">
              <span>User id</span>
              <input
                value={userId}
                onChange={(e) => setUserId(e.target.value)}
                required
              />
            </label>
          )}
          <label className="field">
            <span>Role</span>
            <select
              value={role}
              onChange={(e) => setRole(e.target.value as ProjectRole)}
            >
              {PROJECT_ROLES.map((r) => (
                <option key={r} value={r}>
                  {roleDisplayName(r)}
                </option>
              ))}
            </select>
          </label>
          <button className="btn" type="submit">
            Save membership
          </button>
        </form>
      )}
    </div>
  )
}

function ProjectsPanel({
  projects,
  onCreated,
  onError,
}: {
  projects: Project[]
  onCreated: () => void
  onError: (msg: string) => void
}) {
  const [key, setKey] = useState('')
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault()
    try {
      await api.createProject({
        ...(key.trim() ? { key: key.trim() } : {}),
        name,
        description,
      })
      setKey('')
      setName('')
      setDescription('')
      onCreated()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    }
  }

  return (
    <div className="panel admin-grid">
      <div>
        <h3>Projects</h3>
        <table className="table">
          <thead>
            <tr>
              <th>Key</th>
              <th>Name</th>
            </tr>
          </thead>
          <tbody>
            {projects.map((p) => (
              <tr key={p.id}>
                <td>
                  <span className="badge">{p.key}</span>
                </td>
                <td>{p.name}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <form className="stack-form" onSubmit={(e) => void onSubmit(e)}>
        <h3>New project</h3>
        <label className="field">
          <span>Name</span>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            required
          />
        </label>
        <label className="field">
          <span>Key (optional)</span>
          <input
            value={key}
            onChange={(e) => setKey(e.target.value.toUpperCase())}
            pattern="[A-Z][A-Z0-9_]{1,31}"
            placeholder="Derived from name when empty"
          />
        </label>
        <label className="field">
          <span>Description</span>
          <input
            value={description}
            onChange={(e) => setDescription(e.target.value)}
          />
        </label>
        <button className="btn" type="submit">
          Create project
        </button>
      </form>
    </div>
  )
}

function AutomationRepoPanel({
  projectId,
  onError,
}: {
  projectId: string | null
  onError: (msg: string) => void
}) {
  const { t } = useI18n()
  const [provider, setProvider] = useState('github')
  const [remoteUrl, setRemoteUrl] = useState('')
  const [branch, setBranch] = useState('main')
  const [subpath, setSubpath] = useState('')
  const [token, setToken] = useState('')
  const [status, setStatus] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [info, setInfo] = useState<string | null>(null)

  const load = useCallback(async () => {
    if (!projectId) return
    try {
      const repo = await api.getAutomationRepo(projectId)
      setProvider(repo.provider)
      setRemoteUrl(repo.remote_url)
      setBranch(repo.branch)
      setSubpath(repo.subpath)
      setStatus(
        `${repo.index_status} · files ${repo.file_count} · chunks ${repo.chunk_count}` +
          (repo.commit_sha ? ` · ${repo.commit_sha.slice(0, 8)}` : '') +
          (repo.has_token ? ' · token set' : ''),
      )
      setInfo(repo.last_index_error)
    } catch {
      setStatus(null)
      setInfo(null)
    }
  }, [projectId])

  useEffect(() => {
    void load()
  }, [load])

  if (!projectId) {
    return (
      <div className="panel">
        <h3>{t('admin.automationRepo')}</h3>
        <p className="muted">{t('admin.automationRepoNeedProject')}</p>
      </div>
    )
  }

  const onSave = async (e: FormEvent) => {
    e.preventDefault()
    setBusy(true)
    try {
      await api.upsertAutomationRepo(projectId, {
        provider,
        remote_url: remoteUrl,
        branch,
        subpath,
        ...(token ? { token } : {}),
      })
      setToken('')
      await load()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const onTest = async () => {
    setBusy(true)
    try {
      const res = await api.testAutomationRepo(projectId)
      setInfo(res.message)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const onReindex = async () => {
    setBusy(true)
    try {
      const res = await api.reindexAutomationRepo(projectId)
      setInfo(
        res.error
          ? res.error
          : `Indexed ${res.file_count} files / ${res.chunk_count} chunks`,
      )
      await load()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="panel">
      <h3>{t('admin.automationRepo')}</h3>
      <p className="muted">{t('admin.automationRepoHelp')}</p>
      {status && <p className="notice">{status}</p>}
      {info && <p className="muted">{info}</p>}
      <form className="stack-form" onSubmit={(e) => void onSave(e)}>
        <label className="field">
          <span>{t('admin.automationProvider')}</span>
          <select
            value={provider}
            onChange={(e) => setProvider(e.target.value)}
          >
            <option value="github">GitHub</option>
            <option value="gitlab">GitLab</option>
            <option value="generic">Generic HTTPS Git</option>
          </select>
        </label>
        <label className="field">
          <span>{t('admin.automationUrl')}</span>
          <input
            value={remoteUrl}
            onChange={(e) => setRemoteUrl(e.target.value)}
            placeholder="https://github.com/org/repo.git"
            required
          />
        </label>
        <label className="field">
          <span>{t('admin.automationBranch')}</span>
          <input
            value={branch}
            onChange={(e) => setBranch(e.target.value)}
            required
          />
        </label>
        <label className="field">
          <span>{t('admin.automationSubpath')}</span>
          <input
            value={subpath}
            onChange={(e) => setSubpath(e.target.value)}
            placeholder="e2e"
          />
        </label>
        <label className="field">
          <span>{t('admin.automationToken')}</span>
          <input
            type="password"
            value={token}
            onChange={(e) => setToken(e.target.value)}
            autoComplete="off"
            placeholder={t('admin.automationTokenHint')}
          />
        </label>
        <div className="btn-row">
          <button className="btn" type="submit" disabled={busy}>
            {t('admin.automationSave')}
          </button>
          <button
            className="btn secondary"
            type="button"
            disabled={busy}
            onClick={() => void onTest()}
          >
            {t('admin.automationTest')}
          </button>
          <button
            className="btn secondary"
            type="button"
            disabled={busy}
            onClick={() => void onReindex()}
          >
            {t('admin.automationReindex')}
          </button>
        </div>
      </form>
    </div>
  )
}

function IntegrationsPanel({
  items,
  isAdmin,
  onChanged,
  onError,
}: {
  items: IntegrationSetting[]
  isAdmin: boolean
  onChanged: () => void
  onError: (msg: string) => void
}) {
  const [kind, setKind] = useState('openproject')
  const [name, setName] = useState('')

  if (!isAdmin) {
    return null
  }

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault()
    try {
      await api.createAdminIntegration({
        kind,
        name,
        config: {},
        enabled: false,
      })
      setName('')
      onChanged()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    }
  }

  return (
    <div className="panel admin-grid">
      <div>
        <h3>Integrations</h3>
        <table className="table">
          <thead>
            <tr>
              <th>Kind</th>
              <th>Name</th>
              <th>Enabled</th>
            </tr>
          </thead>
          <tbody>
            {items.map((row) => (
              <tr key={row.id}>
                <td>{row.kind}</td>
                <td>{row.name}</td>
                <td>{row.enabled ? 'yes' : 'no'}</td>
              </tr>
            ))}
            {items.length === 0 && (
              <tr>
                <td colSpan={3} className="muted">
                  No integration settings yet
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      <form className="stack-form" onSubmit={(e) => void onSubmit(e)}>
        <h3>Add placeholder</h3>
        <label className="field">
          <span>Kind</span>
          <select value={kind} onChange={(e) => setKind(e.target.value)}>
            <option value="openproject">openproject</option>
            <option value="jira_cloud">jira_cloud</option>
            <option value="jira_dc">jira_dc</option>
          </select>
        </label>
        <label className="field">
          <span>Name</span>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            required
          />
        </label>
        <button className="btn" type="submit">
          Save
        </button>
      </form>
    </div>
  )
}

function LlmPanel({
  projectId,
  providers,
  configs,
  onChanged,
  onError,
}: {
  projectId: string | null
  providers: ProviderKindInfo[]
  configs: LlmConfig[]
  onChanged: () => void
  onError: (msg: string) => void
}) {
  const [provider, setProvider] = useState('mock')
  const [name, setName] = useState('Default mock')
  const [model, setModel] = useState('mock-1')
  const [baseUrl, setBaseUrl] = useState('')
  const [apiKey, setApiKey] = useState('')
  const [isDefault, setIsDefault] = useState(true)
  const [isGlobal, setIsGlobal] = useState(false)

  useEffect(() => {
    const meta = providers.find((p) => p.id === provider)
    if (meta?.default_base_url && !baseUrl) {
      setBaseUrl(meta.default_base_url)
    }
  }, [provider, providers, baseUrl])

  if (!projectId) {
    return (
      <div className="panel">
        <p className="muted">Select a project to configure LLM providers.</p>
      </div>
    )
  }

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault()
    try {
      await api.createLlmConfig(projectId, {
        provider,
        name,
        model,
        base_url: baseUrl || undefined,
        api_key: apiKey || undefined,
        is_default: isDefault,
        global: isGlobal,
      })
      setApiKey('')
      onChanged()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    }
  }

  return (
    <div className="panel admin-grid">
      <div>
        <h3>LLM providers</h3>
        <p className="muted">
          Resolution: project provider → global default → mock only if none
          exist.
        </p>
        <ul className="plain-list">
          {configs.map((c) => (
            <li key={c.id}>
              <strong>{c.name}</strong> · {c.provider}
              {c.model ? ` · ${c.model}` : ''}
              {c.project_id == null ? ' · global' : ''}
              {c.is_default ? ' · default' : ''}
              {c.has_api_key ? ' · key set' : ''}
              <button
                type="button"
                className="linkish"
                onClick={() =>
                  void api.deleteLlmConfig(projectId, c.id).then(onChanged)
                }
              >
                Delete
              </button>
            </li>
          ))}
          {configs.length === 0 && (
            <li className="muted">
              No providers yet — mock is used until you add one.
            </li>
          )}
        </ul>
      </div>
      <form className="stack-form" onSubmit={(e) => void onSubmit(e)}>
        <h3>Save provider</h3>
        <label className="field">
          <span>Provider</span>
          <select
            value={provider}
            onChange={(e) => setProvider(e.target.value)}
          >
            {providers.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Name</span>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            required
          />
        </label>
        <label className="field">
          <span>Model</span>
          <input value={model} onChange={(e) => setModel(e.target.value)} />
        </label>
        <label className="field">
          <span>Base URL</span>
          <input value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} />
        </label>
        <label className="field">
          <span>API key</span>
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            autoComplete="off"
          />
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={isDefault}
            onChange={(e) => setIsDefault(e.target.checked)}
          />
          {isGlobal ? 'Global default' : 'Default for this project'}
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={isGlobal}
            onChange={(e) => setIsGlobal(e.target.checked)}
          />
          Global (all projects)
        </label>
        <button className="btn" type="submit">
          Save provider
        </button>
      </form>
    </div>
  )
}

function TokensPanel({
  tokens,
  secretOnce,
  onSecret,
  onChanged,
  onError,
}: {
  tokens: ApiTokenPublic[]
  secretOnce: string | null
  onSecret: (secret: string | null) => void
  onChanged: () => void
  onError: (msg: string) => void
}) {
  const [tokenName, setTokenName] = useState('mcp-local')

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault()
    try {
      const res = await api.createToken(tokenName)
      onSecret(res.secret)
      onChanged()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    }
  }

  return (
    <div className="panel admin-grid">
      <div>
        <h3>API tokens (MCP / CI)</h3>
        <p className="muted">
          Use as <code>VULYK_API_TOKEN</code>. Secrets are shown once.
        </p>
        {secretOnce && (
          <p className="token-secret">
            Copy now: <code>{secretOnce}</code>
          </p>
        )}
        <ul className="plain-list">
          {tokens.map((t) => (
            <li key={t.id}>
              <strong>{t.name}</strong> · {t.token_prefix}…
              {t.revoked_at ? ' · revoked' : ''}
              {!t.revoked_at && (
                <button
                  type="button"
                  className="linkish"
                  onClick={() => void api.revokeToken(t.id).then(onChanged)}
                >
                  Revoke
                </button>
              )}
            </li>
          ))}
        </ul>
      </div>
      <form className="stack-form" onSubmit={(e) => void onSubmit(e)}>
        <h3>Create token</h3>
        <label className="field">
          <span>Name</span>
          <input
            value={tokenName}
            onChange={(e) => setTokenName(e.target.value)}
            required
          />
        </label>
        <button className="btn" type="submit">
          Generate
        </button>
      </form>
    </div>
  )
}

function SettingsPanel({
  settings,
  onChanged,
  onError,
}: {
  settings: InstanceSettings | null
  onChanged: () => void
  onError: (msg: string) => void
}) {
  const { t } = useI18n()
  const [instanceName, setInstanceName] = useState('')
  const [baseUrl, setBaseUrl] = useState('')
  const [locale, setLocale] = useState('en')
  const [timezone, setTimezone] = useState('UTC')
  const [defaultTheme, setDefaultTheme] = useState('system')
  const [registrationMode, setRegistrationMode] =
    useState<RegistrationMode>('disabled')
  const [defaultRole, setDefaultRole] = useState<ProjectRole>('viewer')

  useEffect(() => {
    if (!settings) return
    setInstanceName(settings.instance_name)
    setBaseUrl(settings.base_url)
    setLocale(settings.default_locale)
    setTimezone(settings.default_timezone)
    setDefaultTheme(settings.default_theme || 'system')
    setRegistrationMode(settings.registration_mode)
    setDefaultRole(settings.default_role)
  }, [settings])

  if (!settings) {
    return (
      <div className="panel">
        <p className="muted">Loading settings…</p>
      </div>
    )
  }

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault()
    try {
      await api.updateAdminSettings({
        instance_name: instanceName,
        base_url: baseUrl,
        default_locale: locale,
        default_timezone: timezone,
        default_theme: defaultTheme,
        registration_mode: registrationMode,
        default_role: defaultRole,
      })
      onChanged()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    }
  }

  return (
    <div className="panel">
      <h3>{t('admin.settings')}</h3>
      <form className="stack-form" onSubmit={(e) => void onSubmit(e)}>
        <label className="field">
          <span>{t('setup.instanceName')}</span>
          <input
            value={instanceName}
            onChange={(e) => setInstanceName(e.target.value)}
          />
        </label>
        <label className="field">
          <span>{t('setup.baseUrl')}</span>
          <input value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} />
        </label>
        <label className="field">
          <span>{t('setup.locale')}</span>
          <select value={locale} onChange={(e) => setLocale(e.target.value)}>
            <option value="en">English</option>
            <option value="uk">Українська</option>
          </select>
        </label>
        <label className="field">
          <span>{t('setup.timezone')}</span>
          <input
            value={timezone}
            onChange={(e) => setTimezone(e.target.value)}
          />
        </label>
        <label className="field">
          <span>{t('admin.defaultTheme')}</span>
          <select
            value={defaultTheme}
            onChange={(e) => setDefaultTheme(e.target.value)}
          >
            <option value="system">{t('appearance.theme.system')}</option>
            <option value="light">{t('appearance.theme.light')}</option>
            <option value="dark">{t('appearance.theme.dark')}</option>
            <option value="high_contrast">
              {t('appearance.theme.high_contrast')}
            </option>
            <option value="sepia">{t('appearance.theme.sepia')}</option>
          </select>
        </label>
        <label className="field">
          <span>{t('setup.registrationMode')}</span>
          <select
            value={registrationMode}
            onChange={(e) =>
              setRegistrationMode(e.target.value as RegistrationMode)
            }
          >
            <option value="open">{t('setup.modeOpen')}</option>
            <option value="invite">{t('setup.modeInvite')}</option>
            <option value="disabled">{t('setup.modeDisabled')}</option>
          </select>
        </label>
        <label className="field">
          <span>Default project role</span>
          <select
            value={defaultRole}
            onChange={(e) => setDefaultRole(e.target.value as ProjectRole)}
          >
            {PROJECT_ROLES.map((r) => (
              <option key={r} value={r}>
                {roleDisplayName(r)}
              </option>
            ))}
          </select>
        </label>
        <button className="btn" type="submit">
          {t('common.save')}
        </button>
      </form>
    </div>
  )
}

function SsoPanel({
  providers,
  onChanged,
  onError,
}: {
  providers: SsoProviderAdmin[]
  onChanged: () => void
  onError: (msg: string) => void
}) {
  const { t } = useI18n()

  const toggleEnabled = async (p: SsoProviderAdmin) => {
    try {
      await api.updateSsoProvider(p.id, { enabled: !p.enabled })
      onChanged()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    }
  }

  return (
    <div className="panel">
      <h3>{t('admin.sso')}</h3>
      <p className="muted">
        Enable or disable configured OIDC providers. Client secrets are managed
        server-side.
      </p>
      <table className="table">
        <thead>
          <tr>
            <th>Key</th>
            <th>Name</th>
            <th>Client id</th>
            <th>Enabled</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {providers.map((p) => (
            <tr key={p.id}>
              <td>
                <code>{p.provider_key}</code>
              </td>
              <td>{p.display_name}</td>
              <td className="muted">{p.client_id || '—'}</td>
              <td>{p.enabled ? 'yes' : 'no'}</td>
              <td>
                <button
                  type="button"
                  className="btn secondary"
                  onClick={() => void toggleEnabled(p)}
                >
                  {p.enabled ? 'Disable' : 'Enable'}
                </button>
              </td>
            </tr>
          ))}
          {providers.length === 0 && (
            <tr>
              <td colSpan={5} className="muted">
                No SSO providers configured yet.
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  )
}
