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
  type SampleDataStatus,
  type SsoProviderAdmin,
  type UpdateJob,
  type UpdatesStatus,
  type UserPublic,
} from '../api'
import { useAuth } from '../auth'
import { PasswordRequirements } from '../components/PasswordRequirements'
import { SampleBadge } from '../components/SampleBadge'
import { useProjectPermissions } from '../hooks/useProjectPermissions'
import { useI18n } from '../i18n'
import { DEFAULT_PASSWORD_POLICY } from '../passwordPolicy'
import { PROJECT_ROLES, roleDisplayName } from '../permissions'
import { TestIds, listItemProps, testIdProps } from '../testing'
import { getStoredToken } from '../token'
import { BackupsPanel } from './admin/BackupsPanel'

type Tab =
  | 'users'
  | 'roles'
  | 'projects'
  | 'integrations'
  | 'llm'
  | 'tokens'
  | 'settings'
  | 'updates'
  | 'sso'
  | 'backups'

/** Maps admin tab ids → Playwright test ids. Reserved future tabs live in TestIds. */
const ADMIN_TAB_TEST_IDS: Record<Tab, string> = {
  users: TestIds.adminTabUsers,
  roles: TestIds.adminTabRoles,
  projects: TestIds.adminTabProjects,
  integrations: TestIds.adminTabIntegrations,
  llm: TestIds.adminTabLlm,
  tokens: TestIds.adminTabTokens,
  settings: TestIds.adminTabSettings,
  updates: TestIds.adminTabUpdates,
  sso: TestIds.adminTabSso,
  backups: TestIds.adminTabBackups,
}

/** Reserved for upcoming admin tabs/actions (keep imported so keys stay discoverable). */
export const ADMIN_RESERVED_TEST_IDS = {
  tabLists: TestIds.adminTabLists,
  tabCustomFields: TestIds.adminTabCustomFields,
  tabConfig: TestIds.adminTabConfig,
  listsAddButton: TestIds.adminListsAddButton,
  customFieldsAddButton: TestIds.adminCustomFieldsAddButton,
  configExportButton: TestIds.adminConfigExportButton,
  configImportButton: TestIds.adminConfigImportButton,
  configImportInput: TestIds.adminConfigImportInput,
} as const

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
      list.push({ id: 'updates', label: t('admin.updates') })
      list.push({ id: 'sso', label: t('admin.sso') })
      list.push({ id: 'backups', label: t('admin.backups') })
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
    <div className="admin" {...testIdProps(TestIds.adminPage)}>
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
              {...testIdProps(ADMIN_TAB_TEST_IDS[id])}
            >
              {label}
            </button>
          ))}
        </div>
        {error && (
          <p className="error" {...testIdProps(TestIds.adminError)}>
            {error}
          </p>
        )}
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
      {tab === 'updates' && isSystemAdmin && (
        <UpdatesPanel onError={setError} />
      )}
      {tab === 'sso' && isSystemAdmin && (
        <SsoPanel
          providers={ssoProviders}
          onChanged={load}
          onError={setError}
        />
      )}
      {tab === 'backups' && isSystemAdmin && (
        <BackupsPanel projects={projects} onError={setError} />
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
  const { status } = useAuth()
  const policy = status?.password_policy ?? DEFAULT_PASSWORD_POLICY
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
        <table className="table" {...testIdProps(TestIds.adminUserList)}>
          <thead>
            <tr>
              <th>Email</th>
              <th>Name</th>
              <th>Admin</th>
            </tr>
          </thead>
          <tbody>
            {users.map((u, index) => (
              <tr
                key={u.id}
                {...listItemProps(TestIds.adminUserRow, {
                  // UUID only — never put email/PII in data-entity-*
                  entityId: u.id,
                  index,
                })}
              >
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
            {...testIdProps(TestIds.adminUserEmailInput)}
          />
        </label>
        <label className="field">
          <span>Display name</span>
          <input
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
            required
            {...testIdProps(TestIds.adminUserDisplayNameInput)}
          />
        </label>
        <label className="field">
          <span>Password</span>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            minLength={policy.min_length}
            required
            {...testIdProps(TestIds.adminUserPasswordInput)}
          />
          <PasswordRequirements password={password} policy={policy} />
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={systemAdmin}
            onChange={(e) => setSystemAdmin(e.target.checked)}
            {...testIdProps(TestIds.adminUserSystemAdminCheckbox)}
          />
          System admin
        </label>
        <button
          className="btn"
          type="submit"
          {...testIdProps(TestIds.adminUserCreateButton)}
        >
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
            {memberships.map((m, index) => (
              <tr
                key={m.id}
                {...listItemProps(TestIds.adminMembershipRow, {
                  entityId: m.id,
                  index,
                })}
              >
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
                {...testIdProps(TestIds.adminRoleUserSelect)}
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
                {...testIdProps(TestIds.adminRoleUserSelect)}
              />
            </label>
          )}
          <label className="field">
            <span>Role</span>
            <select
              value={role}
              onChange={(e) => setRole(e.target.value as ProjectRole)}
              {...testIdProps(TestIds.adminRoleSelect)}
            >
              {PROJECT_ROLES.map((r) => (
                <option key={r} value={r}>
                  {roleDisplayName(r)}
                </option>
              ))}
            </select>
          </label>
          <button
            className="btn"
            type="submit"
            {...testIdProps(TestIds.adminRoleSaveButton)}
          >
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
        <table className="table" {...testIdProps(TestIds.adminProjectList)}>
          <thead>
            <tr>
              <th>Key</th>
              <th>Name</th>
            </tr>
          </thead>
          <tbody>
            {projects.map((p, index) => (
              <tr
                key={p.id}
                {...listItemProps(TestIds.adminProjectRow, {
                  entityId: p.id,
                  entityKey: p.key,
                  index,
                })}
              >
                <td>
                  <span className="badge">{p.key}</span>
                  {p.is_sample ? <SampleBadge testId="admin-project-sample-badge" /> : null}
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
            {...testIdProps(TestIds.adminProjectNameInput)}
          />
        </label>
        <label className="field">
          <span>Key (optional)</span>
          <input
            value={key}
            onChange={(e) => setKey(e.target.value.toUpperCase())}
            pattern="[A-Z][A-Z0-9_]{1,31}"
            placeholder="Derived from name when empty"
            {...testIdProps(TestIds.adminProjectKeyInput)}
          />
        </label>
        <label className="field">
          <span>Description</span>
          <input
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            {...testIdProps(TestIds.adminProjectDescriptionInput)}
          />
        </label>
        <button
          className="btn"
          type="submit"
          {...testIdProps(TestIds.adminProjectCreateButton)}
        >
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
            {...testIdProps(TestIds.adminAutomationProviderSelect)}
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
            {...testIdProps(TestIds.adminAutomationUrlInput)}
          />
        </label>
        <label className="field">
          <span>{t('admin.automationBranch')}</span>
          <input
            value={branch}
            onChange={(e) => setBranch(e.target.value)}
            required
            {...testIdProps(TestIds.adminAutomationBranchInput)}
          />
        </label>
        <label className="field">
          <span>{t('admin.automationSubpath')}</span>
          <input
            value={subpath}
            onChange={(e) => setSubpath(e.target.value)}
            placeholder="e2e"
            {...testIdProps(TestIds.adminAutomationSubpathInput)}
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
            {...testIdProps(TestIds.adminAutomationTokenInput)}
          />
        </label>
        <div className="btn-row">
          <button
            className="btn"
            type="submit"
            disabled={busy}
            {...testIdProps(TestIds.adminAutomationSaveButton)}
          >
            {t('admin.automationSave')}
          </button>
          <button
            className="btn secondary"
            type="button"
            disabled={busy}
            onClick={() => void onTest()}
            {...testIdProps(TestIds.adminAutomationTestButton)}
          >
            {t('admin.automationTest')}
          </button>
          <button
            className="btn secondary"
            type="button"
            disabled={busy}
            onClick={() => void onReindex()}
            {...testIdProps(TestIds.adminAutomationReindexButton)}
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
        <table className="table" {...testIdProps(TestIds.adminIntegrationList)}>
          <thead>
            <tr>
              <th>Kind</th>
              <th>Name</th>
              <th>Enabled</th>
            </tr>
          </thead>
          <tbody>
            {items.map((row, index) => (
              <tr
                key={row.id}
                {...listItemProps(TestIds.adminIntegrationRow, {
                  entityId: row.id,
                  entityKey: row.kind,
                  index,
                })}
              >
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
          <select
            value={kind}
            onChange={(e) => setKind(e.target.value)}
            {...testIdProps(TestIds.adminIntegrationKindSelect)}
          >
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
            {...testIdProps(TestIds.adminIntegrationNameInput)}
          />
        </label>
        <button
          className="btn"
          type="submit"
          {...testIdProps(TestIds.adminIntegrationCreateButton)}
        >
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
        <ul className="plain-list" {...testIdProps(TestIds.adminLlmList)}>
          {configs.map((c, index) => (
            <li
              key={c.id}
              {...listItemProps(TestIds.adminLlmRow, {
                entityId: c.id,
                index,
              })}
            >
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
                {...testIdProps(TestIds.adminLlmDeleteButton)}
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
            {...testIdProps(TestIds.adminLlmProviderSelect)}
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
            {...testIdProps(TestIds.adminLlmNameInput)}
          />
        </label>
        <label className="field">
          <span>Model</span>
          <input
            value={model}
            onChange={(e) => setModel(e.target.value)}
            {...testIdProps(TestIds.adminLlmModelInput)}
          />
        </label>
        <label className="field">
          <span>Base URL</span>
          <input
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            {...testIdProps(TestIds.adminLlmBaseUrlInput)}
          />
        </label>
        <label className="field">
          <span>API key</span>
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            autoComplete="off"
            {...testIdProps(TestIds.adminLlmApiKeyInput)}
          />
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={isDefault}
            onChange={(e) => setIsDefault(e.target.checked)}
            {...testIdProps(TestIds.adminLlmDefaultCheckbox)}
          />
          {isGlobal ? 'Global default' : 'Default for this project'}
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={isGlobal}
            onChange={(e) => setIsGlobal(e.target.checked)}
            {...testIdProps(TestIds.adminLlmGlobalCheckbox)}
          />
          Global (all projects)
        </label>
        <button
          className="btn"
          type="submit"
          {...testIdProps(TestIds.adminLlmSaveButton)}
        >
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
          <p className="token-secret" {...testIdProps(TestIds.adminTokenSecret)}>
            Copy now: <code>{secretOnce}</code>
          </p>
        )}
        <ul className="plain-list" {...testIdProps(TestIds.adminTokenList)}>
          {tokens.map((tok, index) => (
            <li
              key={tok.id}
              {...listItemProps(TestIds.adminTokenRow, {
                entityId: tok.id,
                index,
              })}
            >
              <strong>{tok.name}</strong> · {tok.token_prefix}…
              {tok.revoked_at ? ' · revoked' : ''}
              {!tok.revoked_at && (
                <button
                  type="button"
                  className="linkish"
                  onClick={() => void api.revokeToken(tok.id).then(onChanged)}
                  {...testIdProps(TestIds.adminTokenRevokeButton)}
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
            {...testIdProps(TestIds.adminTokenNameInput)}
          />
        </label>
        <button
          className="btn"
          type="submit"
          {...testIdProps(TestIds.adminTokenCreateButton)}
        >
          Generate
        </button>
      </form>
    </div>
  )
}

function UpdatesPanel({ onError }: { onError: (msg: string) => void }) {
  const { t } = useI18n()
  const [status, setStatus] = useState<UpdatesStatus | null>(null)
  const [jobs, setJobs] = useState<UpdateJob[]>([])
  const [checking, setChecking] = useState(false)
  const [applying, setApplying] = useState(false)
  const [rollingBack, setRollingBack] = useState(false)
  const [intervalHours, setIntervalHours] = useState(24)
  const [activeJob, setActiveJob] = useState<UpdateJob | null>(null)

  const refresh = useCallback(async () => {
    try {
      const [s, j] = await Promise.all([
        api.getAdminUpdates(),
        api.listAdminUpdateJobs(),
      ])
      setStatus(s)
      setJobs(j.items)
      setIntervalHours(s.check_interval_hours)
      setActiveJob(s.active_job)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    }
  }, [onError])

  useEffect(() => {
    void refresh()
  }, [refresh])

  // Periodic check when interval > 0 and last check is stale
  useEffect(() => {
    if (!status || status.check_interval_hours <= 0) return
    const last = status.last_checked_at
      ? Date.parse(status.last_checked_at)
      : 0
    const due =
      !last ||
      Date.now() - last > status.check_interval_hours * 3600 * 1000
    if (!due) return
    void (async () => {
      try {
        const s = await api.checkAdminUpdates()
        setStatus(s)
        setActiveJob(s.active_job)
      } catch {
        // Silent periodic failure; user can click Check
      }
    })()
  }, [status?.check_interval_hours, status?.last_checked_at])

  // Poll active job
  useEffect(() => {
    if (!activeJob) return
    if (!['pending', 'running'].includes(activeJob.status)) return
    const id = window.setInterval(() => {
      void (async () => {
        try {
          const job = await api.getAdminUpdateJob(activeJob.id)
          setActiveJob(job)
          if (!['pending', 'running'].includes(job.status)) {
            await refresh()
          }
        } catch {
          /* ignore transient poll errors */
        }
      })()
    }, 2000)
    return () => window.clearInterval(id)
  }, [activeJob, refresh])

  const onCheck = async () => {
    setChecking(true)
    try {
      const s = await api.checkAdminUpdates()
      setStatus(s)
      setActiveJob(s.active_job)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setChecking(false)
    }
  }

  const onApply = async () => {
    if (!status?.latest_version) return
    const version = status.latest_version
    if (!window.confirm(t('admin.updates.confirm').replace('{version}', version))) {
      return
    }
    setApplying(true)
    try {
      const res = await api.applyAdminUpdate(version)
      setActiveJob(res.job)
      await refresh()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setApplying(false)
    }
  }

  const onRollback = async () => {
    if (!status?.rollback) return
    const version = status.rollback.version
    const typed = window.prompt(
      t('admin.updates.rollbackPrompt').replace('{version}', version),
      '',
    )
    if (typed !== 'ROLLBACK') {
      if (typed !== null) {
        onError(t('admin.updates.rollbackConfirm'))
      }
      return
    }
    setRollingBack(true)
    try {
      const res = await api.applyAdminRollback({
        confirm: 'ROLLBACK',
        version,
      })
      setActiveJob(res.job)
      await refresh()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setRollingBack(false)
    }
  }

  const formatBytes = (n: number) => {
    if (n < 1024) return `${n} B`
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
    return `${(n / (1024 * 1024)).toFixed(1)} MB`
  }

  const onDownloadBackup = async (backupId: string) => {
    try {
      const url = api.downloadAdminVersionBackupUrl(backupId)
      const token = getStoredToken()
      const res = await fetch(url, {
        headers: token ? { Authorization: `Bearer ${token}` } : {},
        credentials: 'include',
      })
      if (!res.ok) {
        throw new Error(`download failed (${res.status})`)
      }
      const blob = await res.blob()
      const objectUrl = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = objectUrl
      a.download = `${backupId}.tar.gz`
      a.click()
      URL.revokeObjectURL(objectUrl)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    }
  }

  const onSaveInterval = async (e: FormEvent) => {
    e.preventDefault()
    try {
      const prefs = await api.updateAdminUpdatesPreferences({
        check_interval_hours: intervalHours,
      })
      setIntervalHours(prefs.check_interval_hours)
      await refresh()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    }
  }

  if (!status) {
    return (
      <div className="panel">
        <p className="muted" {...testIdProps(TestIds.adminLoading)}>
          Loading…
        </p>
      </div>
    )
  }

  const jobBusy = Boolean(
    activeJob && ['pending', 'running'].includes(activeJob.status),
  )
  const canApply =
    status.capability === 'compose_updater' &&
    status.update_available &&
    Boolean(status.latest_version) &&
    !applying &&
    !rollingBack &&
    !jobBusy
  const canRollback =
    status.capability === 'compose_updater' &&
    Boolean(status.rollback) &&
    !applying &&
    !rollingBack &&
    !jobBusy

  return (
    <div className="panel">
      <h3>{t('admin.updates')}</h3>
      <p className="muted">
        {status.update_available
          ? t('admin.updates.available')
          : t('admin.updates.uptoDate')}
        {status.last_checked_at
          ? ` · ${t('admin.updates.lastChecked')}: ${status.last_checked_at}`
          : null}
      </p>

      <dl className="admin-updates-meta">
        <div>
          <dt>{t('admin.updates.current')}</dt>
          <dd {...testIdProps(TestIds.adminUpdatesCurrentVersion)}>
            {status.current_version}
          </dd>
        </div>
        <div>
          <dt>{t('admin.updates.latest')}</dt>
          <dd {...testIdProps(TestIds.adminUpdatesLatestVersion)}>
            {status.latest_version ?? '—'}
            {status.latest_url ? (
              <>
                {' '}
                <a
                  href={status.latest_url}
                  target="_blank"
                  rel="noreferrer"
                  {...testIdProps(TestIds.adminUpdatesReleaseLink)}
                >
                  GitHub
                </a>
              </>
            ) : null}
          </dd>
        </div>
      </dl>

      <div className="row gap">
        <button
          type="button"
          className="btn secondary"
          disabled={checking}
          onClick={() => void onCheck()}
          {...testIdProps(TestIds.adminUpdatesCheckButton)}
        >
          {checking ? t('admin.updates.checking') : t('admin.updates.check')}
        </button>
        {status.capability === 'compose_updater' ? (
          <button
            type="button"
            className="btn"
            disabled={!canApply}
            onClick={() => void onApply()}
            {...testIdProps(TestIds.adminUpdatesApplyButton)}
          >
            {applying ? t('admin.updates.applying') : t('admin.updates.apply')}
          </button>
        ) : null}
        {status.capability === 'compose_updater' && status.rollback ? (
          <button
            type="button"
            className="btn secondary"
            disabled={!canRollback}
            onClick={() => void onRollback()}
            {...testIdProps(TestIds.adminUpdatesRollbackButton)}
          >
            {rollingBack
              ? t('admin.updates.rollingBack')
              : t('admin.updates.rollback').replace(
                  '{version}',
                  status.rollback.version,
                )}
          </button>
        ) : null}
      </div>

      {status.rollback ? (
        <p
          className="muted"
          {...testIdProps(TestIds.adminUpdatesRollbackExpiry)}
        >
          {t('admin.updates.rollbackExpiry').replace(
            '{date}',
            status.rollback.expires_at ?? '—',
          )}
        </p>
      ) : null}

      {status.capability !== 'compose_updater' ? (
        <div className="admin-updates-manual">
          <h4>{t('admin.updates.manual')}</h4>
          <p className="muted">{t('admin.updates.manualHint')}</p>
          <pre {...testIdProps(TestIds.adminUpdatesManualCommand)}>
            {status.manual_command}
          </pre>
          <h4>{t('admin.updates.rollbackManual')}</h4>
          <p className="muted">{t('admin.updates.rollbackManualHint')}</p>
          <pre {...testIdProps(TestIds.adminUpdatesRollbackCommand)}>
            {status.rollback_command}
          </pre>
        </div>
      ) : null}

      {status.latest_notes ? (
        <div>
          <h4>{t('admin.updates.notes')}</h4>
          <pre
            className="admin-updates-notes"
            {...testIdProps(TestIds.adminUpdatesNotes)}
          >
            {status.latest_notes}
          </pre>
        </div>
      ) : null}

      {activeJob ? (
        <div>
          <h4>{t('admin.updates.progress')}</h4>
          <p {...testIdProps(TestIds.adminUpdatesJobStatus)}>
            {activeJob.status} — {activeJob.message} ({activeJob.progress}%)
          </p>
          <progress
            max={100}
            value={activeJob.progress}
            {...testIdProps(TestIds.adminUpdatesProgress)}
          />
          <pre {...testIdProps(TestIds.adminUpdatesJobLog)}>
            {activeJob.log_text || '(no log yet)'}
          </pre>
        </div>
      ) : null}

      <form className="row gap" onSubmit={(e) => void onSaveInterval(e)}>
        <label>
          {t('admin.updates.interval')}
          <input
            type="number"
            min={0}
            max={720}
            value={intervalHours}
            onChange={(e) => setIntervalHours(Number(e.target.value))}
            {...testIdProps(TestIds.adminUpdatesIntervalInput)}
          />
        </label>
        <button
          type="submit"
          className="btn secondary"
          {...testIdProps(TestIds.adminUpdatesIntervalSaveButton)}
        >
          {t('admin.updates.intervalSave')}
        </button>
      </form>

      <h4>{t('admin.updates.backups')}</h4>
      <p className="muted">
        {t('admin.updates.backupsHint')
          .replace('{count}', String(status.retention.version_count))
          .replace('{days}', String(status.retention.rollback_days))}
      </p>
      <table
        className="table"
        {...testIdProps(TestIds.adminUpdatesBackupList)}
      >
        <thead>
          <tr>
            <th>{t('admin.updates.backupVersion')}</th>
            <th>{t('admin.updates.backupDate')}</th>
            <th>{t('admin.updates.backupSize')}</th>
            <th>{t('admin.updates.backupExpiry')}</th>
            <th>{t('admin.updates.backupRoles')}</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {(status.version_backups ?? []).length === 0 ? (
            <tr>
              <td colSpan={6} className="muted">
                —
              </td>
            </tr>
          ) : (
            status.version_backups.map((b, index) => (
              <tr
                key={b.id}
                {...listItemProps(TestIds.adminUpdatesBackupRow, {
                  entityId: b.id,
                  entityKey: b.version ?? b.id,
                  index,
                })}
              >
                <td>{b.version ?? '—'}</td>
                <td>{b.created_at ?? '—'}</td>
                <td>{formatBytes(b.size_bytes)}</td>
                <td>{b.expires_at ?? '—'}</td>
                <td>{(b.roles ?? []).join(', ') || '—'}</td>
                <td>
                  {status.capability === 'compose_updater' ? (
                    <button
                      type="button"
                      className="btn secondary"
                      onClick={() => void onDownloadBackup(b.id)}
                      {...listItemProps(TestIds.adminUpdatesBackupDownload, {
                        entityId: b.id,
                        entityKey: b.version ?? b.id,
                        index,
                      })}
                    >
                      {t('admin.updates.backupDownload')}
                    </button>
                  ) : null}
                </td>
              </tr>
            ))
          )}
        </tbody>
      </table>

      <h4>{t('admin.updates.history')}</h4>
      <ul {...testIdProps(TestIds.adminUpdatesJobList)}>
        {jobs.length === 0 ? (
          <li className="muted">—</li>
        ) : (
          jobs.map((job, index) => (
            <li
              key={job.id}
              {...listItemProps(TestIds.adminUpdatesJobRow, {
                entityId: job.id,
                entityKey: job.target_version,
                index,
              })}
            >
              {job.target_version} · {job.status} · {job.message}
            </li>
          ))
        )}
      </ul>
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
        <p className="muted" {...testIdProps(TestIds.adminLoading)}>
          Loading settings…
        </p>
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
            {...testIdProps(TestIds.adminSettingsInstanceNameInput)}
          />
        </label>
        <label className="field">
          <span>{t('setup.baseUrl')}</span>
          <input
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            {...testIdProps(TestIds.adminSettingsBaseUrlInput)}
          />
        </label>
        <label className="field">
          <span>{t('setup.locale')}</span>
          <select
            value={locale}
            aria-label={t('setup.locale')}
            onChange={(e) => setLocale(e.target.value)}
            {...testIdProps(TestIds.adminSettingsLocaleSelect)}
          >
            <option value="en">English</option>
            <option value="uk">Українська</option>
            <option value="fr">Français</option>
            <option value="es">Español</option>
            <option value="de">Deutsch</option>
          </select>
        </label>
        <label className="field">
          <span>{t('setup.timezone')}</span>
          <input
            value={timezone}
            onChange={(e) => setTimezone(e.target.value)}
            {...testIdProps(TestIds.adminSettingsTimezoneInput)}
          />
        </label>
        <label className="field">
          <span>{t('admin.defaultTheme')}</span>
          <select
            value={defaultTheme}
            onChange={(e) => setDefaultTheme(e.target.value)}
            {...testIdProps(TestIds.adminSettingsThemeSelect)}
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
            {...testIdProps(TestIds.adminSettingsRegistrationSelect)}
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
            {...testIdProps(TestIds.adminSettingsDefaultRoleSelect)}
          >
            {PROJECT_ROLES.map((r) => (
              <option key={r} value={r}>
                {roleDisplayName(r)}
              </option>
            ))}
          </select>
        </label>
        <button
          className="btn"
          type="submit"
          {...testIdProps(TestIds.adminSettingsSaveButton)}
        >
          {t('common.save')}
        </button>
      </form>
      <SampleDataPanel onChanged={onChanged} onError={onError} />
    </div>
  )
}

function SampleDataPanel({
  onChanged,
  onError,
}: {
  onChanged: () => void
  onError: (msg: string) => void
}) {
  const { t } = useI18n()
  const [status, setStatus] = useState<SampleDataStatus | null>(null)
  const [busy, setBusy] = useState(false)
  const [notice, setNotice] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    try {
      setStatus(await api.getSampleData())
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    }
  }, [onError])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const onRemove = async () => {
    if (!window.confirm(t('admin.sampleConfirm'))) return
    setBusy(true)
    setNotice(null)
    try {
      await api.removeSampleData()
      setNotice(t('admin.sampleRemoved'))
      await refresh()
      onChanged()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const onReseed = async () => {
    setBusy(true)
    setNotice(null)
    try {
      const next = await api.reseedSampleData()
      setStatus(next)
      setNotice(t('admin.sampleReseeded'))
      onChanged()
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <section
      className="stack-form"
      style={{ marginTop: '1.5rem', paddingTop: '1.25rem', borderTop: '1px solid var(--line)' }}
      data-testid="admin-sample-data-section"
    >
      <h3 style={{ marginTop: 0 }}>{t('admin.sampleSection')}</h3>
      <p className="muted" style={{ marginTop: 0 }}>
        {t('admin.sampleHelp')}
      </p>
      {status?.present ? (
        <>
          <p data-testid="admin-sample-data-present">{t('admin.samplePresent')}</p>
          <ul data-testid="admin-sample-data-list">
            {status.projects.map((p) => (
              <li key={p.id}>
                <code>{p.key}</code> — {p.name} <SampleBadge />
              </li>
            ))}
          </ul>
          <p className="muted" data-testid="admin-sample-data-summary">
            {status.summary.projects} projects · {status.summary.cases} cases ·{' '}
            {status.summary.runs} runs · {status.summary.campaigns} campaigns
          </p>
        </>
      ) : (
        <p className="muted" data-testid="admin-sample-data-empty">
          {t('admin.sampleEmpty')}
        </p>
      )}
      {notice && (
        <p className="notice" data-testid="admin-sample-data-notice">
          {notice}
        </p>
      )}
      <div className="toolbar" style={{ gap: '0.5rem' }}>
        <button
          type="button"
          className="btn danger"
          disabled={busy || !status?.present}
          onClick={() => void onRemove()}
          data-testid="admin-sample-data-remove-button"
        >
          {t('admin.sampleRemove')}
        </button>
        <button
          type="button"
          className="btn secondary"
          disabled={busy}
          onClick={() => void onReseed()}
          data-testid="admin-sample-data-reseed-button"
        >
          {t('admin.sampleReseed')}
        </button>
      </div>
    </section>
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
      <table className="table" {...testIdProps(TestIds.adminSsoList)}>
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
          {providers.map((p, index) => (
            <tr
              key={p.id}
              {...listItemProps(TestIds.adminSsoRow, {
                entityId: p.id,
                entityKey: p.provider_key,
                index,
              })}
            >
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
                  {...testIdProps(TestIds.adminSsoToggleButton)}
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
