import { useCallback, useEffect, useState, type FormEvent } from 'react'
import {
  api,
  type ApiTokenPublic,
  type IntegrationSetting,
  type LlmConfig,
  type MembershipView,
  type Project,
  type ProjectRole,
  type ProviderKindInfo,
  type UserPublic,
} from '../api'
import { useAuth } from '../auth'

type Tab = 'users' | 'roles' | 'projects' | 'integrations' | 'llm' | 'tokens'

const ROLES: ProjectRole[] = ['admin', 'project_lead', 'tester', 'viewer']

type Props = {
  projects: Project[]
  project: Project | null
  onProjectsChanged: () => void
}

export function Admin({ projects, project, onProjectsChanged }: Props) {
  const { user } = useAuth()
  const [tab, setTab] = useState<Tab>('users')
  const [error, setError] = useState<string | null>(null)
  const [users, setUsers] = useState<UserPublic[]>([])
  const [memberships, setMemberships] = useState<MembershipView[]>([])
  const [integrations, setIntegrations] = useState<IntegrationSetting[]>([])
  const [providers, setProviders] = useState<ProviderKindInfo[]>([])
  const [configs, setConfigs] = useState<LlmConfig[]>([])
  const [tokens, setTokens] = useState<ApiTokenPublic[]>([])
  const [secretOnce, setSecretOnce] = useState<string | null>(null)

  const isAdmin = Boolean(user?.is_system_admin)
  const projectId = project?.id ?? null

  const load = useCallback(async () => {
    setError(null)
    try {
      if (isAdmin) {
        setUsers(await api.listUsers())
        setIntegrations(await api.listAdminIntegrations())
      }
      if (projectId) {
        setMemberships(await api.listMemberships(projectId))
        setProviders(await api.listLlmProviders())
        setConfigs(await api.listLlmConfigs(projectId))
      } else {
        setMemberships([])
        setConfigs([])
      }
      setTokens(await api.listTokens())
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [isAdmin, projectId])

  useEffect(() => {
    void load()
  }, [load])

  return (
    <div className="admin">
      <div className="panel">
        <h2>Admin</h2>
        <p className="muted">
          Users, project roles, integrations, LLM providers, and API tokens for
          CI/MCP.
        </p>
        <div className="admin-tabs" role="tablist">
          {(
            [
              ['users', 'Users'],
              ['roles', 'Roles'],
              ['projects', 'Projects'],
              ['integrations', 'Integrations'],
              ['llm', 'LLM providers'],
              ['tokens', 'API tokens'],
            ] as const
          ).map(([id, label]) => (
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
        <UsersPanel users={users} isAdmin={isAdmin} onCreated={load} onError={setError} />
      )}
      {tab === 'roles' && (
        <RolesPanel
          project={project}
          users={users}
          memberships={memberships}
          isAdmin={isAdmin}
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
        <IntegrationsPanel
          items={integrations}
          isAdmin={isAdmin}
          onChanged={load}
          onError={setError}
        />
      )}
      {tab === 'llm' && (
        <LlmPanel
          projectId={projectId}
          providers={providers}
          configs={configs}
          onChanged={load}
          onError={setError}
        />
      )}
      {tab === 'tokens' && (
        <TokensPanel
          tokens={tokens}
          secretOnce={secretOnce}
          onSecret={setSecretOnce}
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
        <p className="muted">System administrator access is required to manage users.</p>
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
      <form className="stack-form" onSubmit={onSubmit}>
        <h3>Add user</h3>
        <label className="field">
          <span>Email</span>
          <input type="email" value={email} onChange={(e) => setEmail(e.target.value)} required />
        </label>
        <label className="field">
          <span>Display name</span>
          <input value={displayName} onChange={(e) => setDisplayName(e.target.value)} required />
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
  onChanged,
  onError,
}: {
  project: Project | null
  users: UserPublic[]
  memberships: MembershipView[]
  isAdmin: boolean
  onChanged: () => void
  onError: (msg: string) => void
}) {
  const [userId, setUserId] = useState('')
  const [role, setRole] = useState<ProjectRole>('tester')

  if (!project) {
    return (
      <div className="panel">
        <p className="muted">Select a project to manage memberships.</p>
      </div>
    )
  }

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault()
    if (!userId) return
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
        <p className="muted">Per-project: admin, project lead, tester, viewer.</p>
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
                  <span className="badge">{m.role}</span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <form className="stack-form" onSubmit={onSubmit}>
        <h3>Assign role</h3>
        {!isAdmin && users.length === 0 && (
          <p className="muted">User directory requires system admin.</p>
        )}
        {users.length > 0 ? (
          <label className="field">
            <span>User</span>
            <select value={userId} onChange={(e) => setUserId(e.target.value)} required>
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
            <input value={userId} onChange={(e) => setUserId(e.target.value)} required />
          </label>
        )}
        <label className="field">
          <span>Role</span>
          <select value={role} onChange={(e) => setRole(e.target.value as ProjectRole)}>
            {ROLES.map((r) => (
              <option key={r} value={r}>
                {r}
              </option>
            ))}
          </select>
        </label>
        <button className="btn" type="submit">
          Save membership
        </button>
      </form>
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
      await api.createProject({ key, name, description })
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
      <form className="stack-form" onSubmit={onSubmit}>
        <h3>New project</h3>
        <label className="field">
          <span>Key</span>
          <input
            value={key}
            onChange={(e) => setKey(e.target.value.toUpperCase())}
            pattern="[A-Z][A-Z0-9_]{1,31}"
            required
          />
        </label>
        <label className="field">
          <span>Name</span>
          <input value={name} onChange={(e) => setName(e.target.value)} required />
        </label>
        <label className="field">
          <span>Description</span>
          <input value={description} onChange={(e) => setDescription(e.target.value)} />
        </label>
        <button className="btn" type="submit">
          Create project
        </button>
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
    return (
      <div className="panel">
        <p className="muted">
          Integration settings are managed by system administrators. Adapters land in W6.
        </p>
      </div>
    )
  }

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault()
    try {
      await api.createAdminIntegration({ kind, name, config: {}, enabled: false })
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
      <form className="stack-form" onSubmit={onSubmit}>
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
          <input value={name} onChange={(e) => setName(e.target.value)} required />
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
        <p className="muted">Keys encrypted at rest (W9).</p>
        <ul className="plain-list">
          {configs.map((c) => (
            <li key={c.id}>
              <strong>{c.name}</strong> · {c.provider}
              {c.model ? ` · ${c.model}` : ''}
              {c.is_default ? ' · default' : ''}
              {c.has_api_key ? ' · key set' : ''}
              <button
                type="button"
                className="linkish"
                onClick={() => void api.deleteLlmConfig(projectId, c.id).then(onChanged)}
              >
                Delete
              </button>
            </li>
          ))}
          {configs.length === 0 && (
            <li className="muted">No providers yet — mock is used until you add one.</li>
          )}
        </ul>
      </div>
      <form className="stack-form" onSubmit={onSubmit}>
        <h3>Save provider</h3>
        <label className="field">
          <span>Provider</span>
          <select value={provider} onChange={(e) => setProvider(e.target.value)}>
            {providers.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Name</span>
          <input value={name} onChange={(e) => setName(e.target.value)} required />
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
          Default for this project
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
      <form className="stack-form" onSubmit={onSubmit}>
        <h3>Create token</h3>
        <label className="field">
          <span>Name</span>
          <input value={tokenName} onChange={(e) => setTokenName(e.target.value)} required />
        </label>
        <button className="btn" type="submit">
          Generate
        </button>
      </form>
    </div>
  )
}
