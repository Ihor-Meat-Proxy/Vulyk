import { getStoredToken } from './token'

const API_BASE = import.meta.env.VITE_API_URL ?? ''

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const token = getStoredToken()
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(init?.headers as Record<string, string> | undefined),
  }
  if (token) {
    headers.Authorization = `Bearer ${token}`
  }

  const res = await fetch(`${API_BASE}${path}`, {
    ...init,
    headers,
    credentials: 'include',
  })
  if (!res.ok) {
    const text = await res.text()
    try {
      const parsed = JSON.parse(text) as {
        error?: { message?: string; code?: string }
      }
      if (parsed?.error?.message) {
        throw new Error(parsed.error.message)
      }
    } catch (err) {
      if (err instanceof Error && err.message !== text) throw err
    }
    throw new Error(text || res.statusText)
  }
  if (res.status === 204) {
    return undefined as T
  }
  return res.json() as Promise<T>
}

export type Page<T> = {
  items: T[]
  total: number
  limit: number
  offset: number
}

export type Project = {
  id: string
  key: string
  name: string
  description: string
  case_key_prefix: string
  next_case_number: number
  created_at: string
  updated_at: string
}

export type Suite = {
  id: string
  project_id: string
  parent_id: string | null
  name: string
  sort_order: number
  case_count?: number
  created_at?: string
  updated_at?: string
}

export type TestCase = {
  id: string
  project_id: string
  suite_id: string | null
  key: string
  title: string
  description: string
  precondition: string
  priority: string
  status: string
  version: number
  external_key?: string | null
  sort_order?: number
  created_at: string
  updated_at: string
}

export type TestStep = {
  id: string
  case_id: string
  position: number
  action: string
  expected_result: string
  data: string
}

export type Tag = {
  id: string
  project_id: string
  name: string
  color: string
  created_at: string
}

export type CustomFieldValue = {
  field_id: string
  name: string
  field_type: string
  value: unknown
}

export type TestCaseDetail = {
  case: TestCase
  steps: TestStep[]
  tags: Tag[]
  custom_fields: CustomFieldValue[]
}

export type CaseVersionSummary = {
  id: string
  case_id: string
  version: number
  change_summary: string
  created_at: string
}

export type CustomField = {
  id: string
  project_id: string
  name: string
  field_type: string
  options: unknown
  required: boolean
  sort_order: number
}

export type TestPlan = {
  id: string
  project_id: string
  name: string
  description: string
  created_at: string
  updated_at: string
}

export type TestPlanDetail = {
  plan: TestPlan
  case_ids: string[]
  case_count: number
}

export type TestRun = {
  id: string
  project_id: string
  plan_id: string | null
  name: string
  description: string
  status: string
  source: string
  external_ref: string | null
  meta: Record<string, unknown>
  started_at: string | null
  finished_at: string | null
  created_at: string
  updated_at: string
}

export type RunProgress = {
  total: number
  untested: number
  passed: number
  failed: number
  blocked: number
  skipped: number
  retest: number
  completed: number
  percent: number
}

export type RunItem = {
  id: string
  run_id: string
  case_id: string
  assignee: string | null
  status: string
  sort_order: number
  comment: string
  executed_at: string | null
  created_at: string
  updated_at: string
}

export type RunItemView = {
  item: RunItem
  case_key: string
  case_title: string
  case_priority: string
}

export type TestRunDetail = {
  run: TestRun
  progress: RunProgress
  items: RunItemView[]
}

export type ResultStep = {
  id: string
  result_id: string
  step_id: string | null
  position: number
  action: string
  expected_result: string
  status: string
  actual_result: string
  comment: string
}

/** Canonical execution statuses (PascalCase), matching the API/DB. */
export const RESULT_STATUSES = [
  'Untested',
  'Passed',
  'Failed',
  'Blocked',
  'Skipped',
  'Retest',
] as const

export type ResultStatus = (typeof RESULT_STATUSES)[number]

export function statusClass(status: string): string {
  return `status-${status.toLowerCase()}`
}

export type TestResult = {
  id: string
  run_id: string
  case_id: string | null
  run_item_id: string | null
  status: string
  message: string
  source: string
  external_id: string | null
  duration_ms: number | null
  assignee: string | null
  elapsed_ms: number | null
  executed_by: string | null
  case_version: number | null
  executed_at: string
  created_at?: string
}

export type TestResultDetail = {
  result: TestResult
  steps: ResultStep[]
}

export type TestResultHistoryItem = {
  result: TestResult
  run_name: string | null
  executed_by_name: string | null
  executed_by_email: string | null
  attachment_count: number
  defect_count: number
  steps: ResultStep[]
}

export type CaseResultsSummary = {
  case_id: string
  latest_status: string | null
  latest_executed_at: string | null
  total_results: number
  passed: number
  failed: number
  blocked: number
  skipped: number
  retest: number
  pass_rate: number
}

export type CaseResultsPage = {
  summary: CaseResultsSummary
  items: TestResultHistoryItem[]
  total: number
  limit: number
  offset: number
}

export type RunItemExecution = {
  item: RunItemView
  steps: TestStep[]
  latest_result: TestResultDetail | null
  history: TestResultHistoryItem[]
}

export type ListCasesParams = {
  suiteId?: string | null
  q?: string
  tag?: string
  status?: string
  priority?: string
  limit?: number
  offset?: number
}

export type UserPreferences = {
  theme: string
  font_size: number
  density: string
  reduced_motion: boolean | null
}

export type UpdatePreferences = {
  theme?: string
  font_size?: number
  density?: string
  reduced_motion?: boolean | null
}

export type UserPublic = {
  id: string
  email: string
  display_name: string
  is_system_admin: boolean
  must_change_password: boolean
  locale: string | null
  timezone: string | null
  preferences: UserPreferences
  disabled_at: string | null
  created_at: string
  updated_at: string
}

export type RegistrationMode = 'open' | 'invite' | 'disabled'

export type SsoProviderPublic = {
  id: string
  provider_key: string
  display_name: string
  enabled: boolean
}

export type AuthStatus = {
  auth_required: boolean
  bootstrap_available: boolean
  setup_required: boolean
  registration_mode: RegistrationMode
  default_locale: string
  default_timezone: string
  default_theme: string
  instance_name: string
  authenticated: boolean
  user: UserPublic | null
  sso_providers: SsoProviderPublic[]
}

export type SetupStatus = {
  setup_completed: boolean
  setup_required: boolean
}

export type CompleteSetupRequest = {
  setup_token: string
  email: string
  password: string
  display_name: string
  instance_name?: string
  base_url?: string
  default_locale?: string
  default_timezone?: string
  default_theme?: string
  registration_mode?: RegistrationMode
  /** Seed Kyiv concert sample project (default true). */
  seed_sample?: boolean
}

export type AuthResponse = {
  token: string
  token_type: string
  expires_in_hours: number
  user: UserPublic
}

export type ProjectRole =
  | 'admin'
  | 'test_architect'
  | 'test_automation_engineer'
  | 'developer'
  | 'viewer'

export type InstanceSettings = {
  setup_completed: boolean
  instance_name: string
  base_url: string
  default_locale: string
  default_timezone: string
  default_theme: string
  registration_mode: RegistrationMode
  default_role: ProjectRole
  allowed_email_domains: string[]
}

export type UpdateInstanceSettings = {
  instance_name?: string
  base_url?: string
  default_locale?: string
  default_timezone?: string
  default_theme?: string
  registration_mode?: RegistrationMode
  default_role?: ProjectRole
  allowed_email_domains?: string[]
}

export type SsoProviderAdmin = {
  id: string
  provider_key: string
  display_name: string
  enabled: boolean
  issuer_url: string | null
  authorization_endpoint: string | null
  token_endpoint: string | null
  userinfo_endpoint: string | null
  jwks_uri: string | null
  client_id: string
  has_client_secret: boolean
  scopes: string
  extra_config: unknown
  default_role: ProjectRole
  group_role_mapping: unknown
  allowed_email_domains: string[]
  created_at: string
  updated_at: string
}

export type UpdateSsoProviderRequest = {
  display_name?: string
  enabled?: boolean
  issuer_url?: string | null
  authorization_endpoint?: string | null
  token_endpoint?: string | null
  userinfo_endpoint?: string | null
  jwks_uri?: string | null
  client_id?: string
  client_secret?: string | null
  scopes?: string
  extra_config?: unknown
  default_role?: ProjectRole
  group_role_mapping?: unknown
  allowed_email_domains?: string[]
}

export type RoleMatrixRow = {
  role: ProjectRole
  display_name: string
  permissions: string[]
}

export type RoleMatrixResponse = {
  roles: RoleMatrixRow[]
}

export type MembershipView = {
  id: string
  project_id: string
  user_id: string
  role: ProjectRole
  email: string
  display_name: string
  created_at: string
  updated_at: string
}

export type IntegrationSetting = {
  id: string
  project_id: string | null
  kind: string
  name: string
  config: Record<string, unknown>
  enabled: boolean
  created_at: string
  updated_at: string
}

export type ProviderKindInfo = {
  id: string
  name: string
  default_base_url: string | null
  requires_api_key: boolean
}

export type LlmConfig = {
  id: string
  project_id: string | null
  provider: string
  name: string
  base_url: string | null
  model: string
  has_api_key: boolean
  is_default: boolean
  created_at: string
  updated_at: string
}

export type GeneratedCaseDraft = {
  key: string
  title: string
  description: string
  precondition: string
  priority: string
}

export type ContextFileRef = {
  path: string
  language: string
  score: number
  reason: string
}

export type AutomationRepo = {
  id: string
  project_id: string
  provider: string
  remote_url: string
  branch: string
  subpath: string
  has_token: boolean
  index_status: string
  last_indexed_at: string | null
  last_index_error: string | null
  commit_sha: string | null
  file_count: number
  chunk_count: number
  created_at: string
  updated_at: string
}

export type ApiTokenPublic = {
  id: string
  name: string
  token_prefix: string
  created_at: string
  last_used_at: string | null
  revoked_at: string | null
}

export type CiConfig = {
  id: string
  project_id: string
  provider: string
  name: string
  config: Record<string, unknown>
  webhook_secret: string | null
  enabled: boolean
}

export type CiDispatch = {
  id: string
  project_id: string
  ci_config_id: string
  run_id: string | null
  status: string
  external_id: string | null
  external_url: string | null
  inputs: Record<string, unknown>
  last_error: string | null
  created_at: string
  updated_at: string
}

export type Requirement = {
  id: string
  project_id: string
  external_key: string | null
  external_id: string | null
  title: string
  description: string
  source: string
  url: string
  created_at: string
  updated_at: string
}

export type ImporterInfo = {
  id: string
  name: string
  supported_formats: string[]
}

export type FieldMapping = {
  title: string
  key: string
  description: string
  precondition: string
  priority: string
  status: string
  suite: string
  steps: string
}

export type ImportCaseDraft = {
  key?: string | null
  title: string
  description: string
  precondition: string
  priority: string
  status: string
  suite_name?: string | null
  steps: { action: string; expected_result: string; data: string }[]
  external_id?: string | null
}

export type ImportPreview = {
  importer_id: string
  format: string
  total: number
  cases: ImportCaseDraft[]
  warnings: string[]
  field_mapping: FieldMapping
}

export type ImportReport = {
  importer_id: string
  created_cases: number
  created_suites: number
  skipped: number
  warnings: string[]
  case_ids: string[]
}

export type IntegrationPublic = {
  id: string
  project_id: string
  kind: string
  name: string
  base_url: string
  config: Record<string, unknown>
  has_secrets: boolean
  enabled: boolean
}

export type Defect = {
  id: string
  project_id: string
  integration_id: string | null
  result_id: string | null
  case_id: string | null
  external_key: string | null
  external_url: string
  title: string
  status: string
}

export type StatusCounts = {
  total: number
  passed: number
  failed: number
  skipped: number
  blocked: number
  error: number
  untested: number
  retest: number
  other: number
}

export type RunSummaryReport = {
  run_id: string
  project_id: string
  name: string
  status: string
  started_at: string | null
  finished_at: string | null
  counts: StatusCounts
  pass_rate: number
}

export type TrendPoint = {
  bucket: string
  run_id: string | null
  run_name: string | null
  counts: StatusCounts
  pass_rate: number
}

export type PassFailTrends = {
  project_id: string
  granularity: string
  points: TrendPoint[]
}

export type RequirementCoverageRow = {
  requirement_id: string
  external_key: string | null
  title: string
  source: string
  linked_cases: number
  cases_with_results: number
  cases_passed: number
  cases_failed: number
  coverage_percent: number
  pass_percent: number
}

export type RequirementCoverageReport = {
  project_id: string
  total_requirements: number
  covered_requirements: number
  coverage_percent: number
  rows: RequirementCoverageRow[]
}

export type FlakyCase = {
  case_id: string
  case_key: string
  title: string
  runs_seen: number
  passes: number
  failures: number
  flake_score: number
  last_statuses: string[]
}

export type FlakyReport = {
  project_id: string
  lookback_runs: number
  cases: FlakyCase[]
}

export type ProjectDashboard = {
  project_id: string
  project_key: string
  project_name: string
  total_cases: number
  total_runs: number
  total_requirements: number
  recent_run_summaries: RunSummaryReport[]
  trends: PassFailTrends
  coverage: RequirementCoverageReport
  flaky: FlakyReport
}

const defaultMapping = (): FieldMapping => ({
  title: 'title',
  key: 'key',
  description: 'description',
  precondition: 'precondition',
  priority: 'priority',
  status: 'status',
  suite: 'suite',
  steps: 'steps',
})

export function ssoStartUrl(providerKey: string): string {
  return `${API_BASE}/api/v1/auth/sso/${encodeURIComponent(providerKey)}/start`
}

export const api = {
  authStatus: () => request<AuthStatus>('/api/v1/auth/status'),
  login: (body: { email: string; password: string }) =>
    request<AuthResponse>('/api/v1/auth/login', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  register: (body: {
    email: string
    password: string
    display_name: string
    invite_token?: string
  }) =>
    request<AuthResponse>('/api/v1/auth/register', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  bootstrap: (body: {
    email: string
    password: string
    display_name: string
  }) =>
    request<AuthResponse>('/api/v1/auth/bootstrap', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  logout: () => request<void>('/api/v1/auth/logout', { method: 'POST' }),
  changePassword: (body: {
    current_password?: string
    new_password: string
  }) =>
    request<void>('/api/v1/auth/change-password', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  updateMe: (body: {
    display_name?: string
    locale?: string
    timezone?: string
    preferences?: UpdatePreferences
  }) =>
    request<UserPublic>('/api/v1/auth/me', {
      method: 'PATCH',
      body: JSON.stringify(body),
    }),
  getMe: () => request<UserPublic>('/api/v1/auth/me'),

  setupStatus: () => request<SetupStatus>('/api/v1/setup/status'),
  completeSetup: (body: CompleteSetupRequest) =>
    request<{ ok: boolean; setup_completed: boolean }>(
      '/api/v1/setup/complete',
      { method: 'POST', body: JSON.stringify(body) },
    ),

  getRoleMatrix: () => request<RoleMatrixResponse>('/api/v1/roles/matrix'),
  getAdminSettings: () =>
    request<InstanceSettings>('/api/v1/admin/settings'),
  updateAdminSettings: (body: UpdateInstanceSettings) =>
    request<InstanceSettings>('/api/v1/admin/settings', {
      method: 'PUT',
      body: JSON.stringify(body),
    }),
  listSsoProvidersAdmin: () =>
    request<SsoProviderAdmin[]>('/api/v1/admin/sso/providers'),
  updateSsoProvider: (id: string, body: UpdateSsoProviderRequest) =>
    request<SsoProviderAdmin>(`/api/v1/admin/sso/providers/${id}`, {
      method: 'PUT',
      body: JSON.stringify(body),
    }),

  listUsers: () => request<UserPublic[]>('/api/v1/users'),
  createUser: (body: {
    email: string
    password: string
    display_name: string
    is_system_admin?: boolean
  }) =>
    request<UserPublic>('/api/v1/users', {
      method: 'POST',
      body: JSON.stringify(body),
    }),

  listMemberships: (projectId: string) =>
    request<MembershipView[]>(`/api/v1/projects/${projectId}/memberships`),
  upsertMembership: (
    projectId: string,
    body: { user_id: string; role: ProjectRole },
  ) =>
    request(`/api/v1/projects/${projectId}/memberships`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),

  listAdminIntegrations: () =>
    request<IntegrationSetting[]>('/api/v1/admin/integrations'),
  createAdminIntegration: (body: {
    kind: string
    name: string
    config?: Record<string, unknown>
    enabled?: boolean
    project_id?: string | null
  }) =>
    request<IntegrationSetting>('/api/v1/admin/integrations', {
      method: 'POST',
      body: JSON.stringify(body),
    }),

  listProjects: () => request<Project[]>('/api/v1/projects'),
  createProject: (body: {
    key?: string
    name: string
    description?: string
    case_key_prefix?: string
  }) =>
    request<Project>('/api/v1/projects', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  updateProject: (
    projectId: string,
    body: {
      name?: string
      description?: string
      case_key_prefix?: string
    },
  ) =>
    request<Project>(`/api/v1/projects/${projectId}`, {
      method: 'PUT',
      body: JSON.stringify(body),
    }),
  listSuites: (projectId: string) =>
    request<Suite[]>(`/api/v1/projects/${projectId}/suites`),
  createSuite: (
    projectId: string,
    body: { name: string; parent_id?: string | null; sort_order?: number },
  ) =>
    request<Suite>(`/api/v1/projects/${projectId}/suites`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  updateSuite: (
    projectId: string,
    suiteId: string,
    body: {
      name?: string
      parent_id?: string | null
      clear_parent?: boolean
      sort_order?: number
    },
  ) =>
    request<Suite>(`/api/v1/projects/${projectId}/suites/${suiteId}`, {
      method: 'PUT',
      body: JSON.stringify(body),
    }),
  deleteSuite: (
    projectId: string,
    suiteId: string,
    onNonempty: 'move_to_parent' | 'delete_cases' = 'move_to_parent',
  ) =>
    request<void>(
      `/api/v1/projects/${projectId}/suites/${suiteId}?on_nonempty=${onNonempty}`,
      { method: 'DELETE' },
    ),
  listCases: (projectId: string, params: ListCasesParams = {}) => {
    const qs = new URLSearchParams()
    if (params.suiteId) qs.set('suite_id', params.suiteId)
    if (params.q) qs.set('q', params.q)
    if (params.tag) qs.set('tag', params.tag)
    if (params.status) qs.set('status', params.status)
    if (params.priority) qs.set('priority', params.priority)
    qs.set('limit', String(params.limit ?? 100))
    qs.set('offset', String(params.offset ?? 0))
    return request<Page<TestCase>>(
      `/api/v1/projects/${projectId}/cases?${qs.toString()}`,
    )
  },
  getCase: (projectId: string, caseId: string) =>
    request<TestCaseDetail>(`/api/v1/projects/${projectId}/cases/${caseId}`),
  updateCase: (
    projectId: string,
    caseId: string,
    body: Record<string, unknown>,
  ) =>
    request<TestCaseDetail>(`/api/v1/projects/${projectId}/cases/${caseId}`, {
      method: 'PUT',
      body: JSON.stringify(body),
    }),
  createCase: (projectId: string, body: Record<string, unknown>) =>
    request<TestCaseDetail>(`/api/v1/projects/${projectId}/cases`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  bulkCases: (projectId: string, body: Record<string, unknown>) =>
    request<{ affected: number; action: string }>(
      `/api/v1/projects/${projectId}/cases/bulk`,
      { method: 'POST', body: JSON.stringify(body) },
    ),
  listVersions: (projectId: string, caseId: string) =>
    request<CaseVersionSummary[]>(
      `/api/v1/projects/${projectId}/cases/${caseId}/versions`,
    ),
  listCaseResults: (
    projectId: string,
    caseId: string,
    params: {
      run_id?: string
      status?: string
      user_id?: string
      from?: string
      to?: string
      limit?: number
      offset?: number
      include_steps?: boolean
    } = {},
  ) => {
    const qs = new URLSearchParams()
    if (params.run_id) qs.set('run_id', params.run_id)
    if (params.status) qs.set('status', params.status)
    if (params.user_id) qs.set('user_id', params.user_id)
    if (params.from) qs.set('from', params.from)
    if (params.to) qs.set('to', params.to)
    if (params.limit != null) qs.set('limit', String(params.limit))
    if (params.offset != null) qs.set('offset', String(params.offset))
    if (params.include_steps) qs.set('include_steps', 'true')
    const q = qs.toString()
    return request<CaseResultsPage>(
      `/api/v1/projects/${projectId}/cases/${caseId}/results${q ? `?${q}` : ''}`,
    )
  },
  listRunItemResults: (projectId: string, runId: string, itemId: string) =>
    request<TestResultHistoryItem[]>(
      `/api/v1/projects/${projectId}/runs/${runId}/items/${itemId}/results`,
    ),
  setCustomFields: (
    projectId: string,
    caseId: string,
    body: { field_id: string; value: unknown }[],
  ) =>
    request<TestCaseDetail>(
      `/api/v1/projects/${projectId}/cases/${caseId}/custom-fields`,
      { method: 'POST', body: JSON.stringify(body) },
    ),
  listTags: (projectId: string) =>
    request<Tag[]>(`/api/v1/projects/${projectId}/tags`),
  createTag: (projectId: string, body: { name: string; color?: string }) =>
    request<Tag>(`/api/v1/projects/${projectId}/tags`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  listCustomFields: (projectId: string) =>
    request<CustomField[]>(`/api/v1/projects/${projectId}/custom-fields`),
  listPlans: (projectId: string) =>
    request<TestPlan[]>(`/api/v1/projects/${projectId}/plans`),
  createPlan: (
    projectId: string,
    body: { name: string; description?: string; case_ids?: string[] },
  ) =>
    request<TestPlanDetail>(`/api/v1/projects/${projectId}/plans`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  listRuns: (projectId: string, limit = 50, offset = 0) =>
    request<Page<TestRun>>(
      `/api/v1/projects/${projectId}/runs?limit=${limit}&offset=${offset}`,
    ),
  getRun: (projectId: string, runId: string) =>
    request<TestRunDetail>(`/api/v1/projects/${projectId}/runs/${runId}`),
  createRun: (projectId: string, body: Record<string, unknown>) =>
    request<TestRunDetail>(`/api/v1/projects/${projectId}/runs`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  updateRun: (projectId: string, runId: string, body: Record<string, unknown>) =>
    request<TestRunDetail>(`/api/v1/projects/${projectId}/runs/${runId}`, {
      method: 'PUT',
      body: JSON.stringify(body),
    }),
  getRunItem: (projectId: string, runId: string, itemId: string) =>
    request<RunItemExecution>(
      `/api/v1/projects/${projectId}/runs/${runId}/items/${itemId}`,
    ),
  updateRunItem: (
    projectId: string,
    runId: string,
    itemId: string,
    body: Record<string, unknown>,
  ) =>
    request<RunItemView>(
      `/api/v1/projects/${projectId}/runs/${runId}/items/${itemId}`,
      { method: 'PUT', body: JSON.stringify(body) },
    ),
  createResult: (
    projectId: string,
    runId: string,
    body: Record<string, unknown>,
  ) =>
    request<TestResultDetail>(
      `/api/v1/projects/${projectId}/runs/${runId}/results/execute`,
      { method: 'POST', body: JSON.stringify(body) },
    ),
  createAttachment: (
    projectId: string,
    body: {
      entity_type: string
      entity_id: string
      filename: string
      content_type?: string
      content_base64: string
    },
  ) =>
    request(`/api/v1/projects/${projectId}/attachments`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),


  listRequirements: (projectId: string) =>
    request<Requirement[]>(`/api/v1/projects/${projectId}/requirements`),
  createRequirement: (
    projectId: string,
    body: { title: string; description?: string; source?: string },
  ) =>
    request<Requirement>(`/api/v1/projects/${projectId}/requirements`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  linkRequirement: (
    projectId: string,
    body: { case_id: string; requirement_id: string },
  ) =>
    request(`/api/v1/projects/${projectId}/requirements:link`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  listCaseRequirements: (projectId: string, caseId: string) =>
    request<Requirement[]>(
      `/api/v1/projects/${projectId}/cases/${caseId}/requirements`,
    ),
  listImporters: () => request<ImporterInfo[]>('/api/v1/importers'),
  previewImport: (
    projectId: string,
    body: {
      importer_id: string
      format: string
      content: string
      field_mapping?: FieldMapping
    },
  ) =>
    request<ImportPreview>(`/api/v1/projects/${projectId}/imports:preview`, {
      method: 'POST',
      body: JSON.stringify({
        field_mapping: defaultMapping(),
        ...body,
      }),
    }),
  runImport: (
    projectId: string,
    body: {
      importer_id: string
      format: string
      content: string
      field_mapping?: FieldMapping
      dry_run?: boolean
    },
  ) =>
    request<ImportPreview | ImportReport>(
      `/api/v1/projects/${projectId}/imports`,
      {
        method: 'POST',
        body: JSON.stringify({
          field_mapping: defaultMapping(),
          dry_run: false,
          ...body,
        }),
      },
    ),
  listIntegrations: (projectId: string) =>
    request<IntegrationPublic[]>(
      `/api/v1/projects/${projectId}/integrations`,
    ),
  listDefects: (projectId: string) =>
    request<Defect[]>(`/api/v1/projects/${projectId}/defects`),

  listLlmProviders: () => request<ProviderKindInfo[]>('/api/v1/llm/providers'),
  listLlmConfigs: (projectId: string) =>
    request<LlmConfig[]>(`/api/v1/projects/${projectId}/llm/configs`),
  createLlmConfig: (
    projectId: string,
    body: {
      provider: string
      name: string
      model?: string
      base_url?: string
      api_key?: string
      is_default?: boolean
      global?: boolean
    },
  ) =>
    request<LlmConfig>(`/api/v1/projects/${projectId}/llm/configs`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  deleteLlmConfig: (projectId: string, configId: string) =>
    request<void>(`/api/v1/projects/${projectId}/llm/configs/${configId}`, {
      method: 'DELETE',
    }),
  generateCases: (
    projectId: string,
    body: {
      requirements_text: string
      count?: number
      persist?: boolean
      suite_id?: string
      config_id?: string
    },
  ) =>
    request<{
      provider: string
      model: string
      drafts: GeneratedCaseDraft[]
      persisted: TestCaseDetail[]
    }>(`/api/v1/projects/${projectId}/llm/generate-cases`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  generateCode: (
    projectId: string,
    body: { case_ids: string[]; framework: string; config_id?: string },
  ) =>
    request<{
      provider: string
      model: string
      framework: string
      code: string
      context_files: ContextFileRef[]
    }>(`/api/v1/projects/${projectId}/llm/generate-code`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  askTests: (projectId: string, question: string) =>
    request<{ provider: string; model: string; answer: string }>(
      `/api/v1/projects/${projectId}/llm/ask`,
      { method: 'POST', body: JSON.stringify({ question }) },
    ),

  getAutomationRepo: (projectId: string) =>
    request<AutomationRepo>(`/api/v1/projects/${projectId}/automation-repo`),
  upsertAutomationRepo: (
    projectId: string,
    body: {
      provider: string
      remote_url: string
      branch?: string
      subpath?: string
      token?: string
    },
  ) =>
    request<AutomationRepo>(`/api/v1/projects/${projectId}/automation-repo`, {
      method: 'PUT',
      body: JSON.stringify(body),
    }),
  deleteAutomationRepo: (projectId: string) =>
    request<void>(`/api/v1/projects/${projectId}/automation-repo`, {
      method: 'DELETE',
    }),
  testAutomationRepo: (projectId: string) =>
    request<{ ok: boolean; message: string; commit_sha: string | null }>(
      `/api/v1/projects/${projectId}/automation-repo/test`,
      { method: 'POST', body: '{}' },
    ),
  reindexAutomationRepo: (projectId: string) =>
    request<{
      status: string
      commit_sha: string | null
      file_count: number
      chunk_count: number
      error: string | null
    }>(`/api/v1/projects/${projectId}/automation-repo/reindex`, {
      method: 'POST',
      body: '{}',
    }),

  listTokens: () => request<ApiTokenPublic[]>('/api/v1/tokens'),
  createToken: (name: string) =>
    request<{ token: ApiTokenPublic; secret: string }>('/api/v1/tokens', {
      method: 'POST',
      body: JSON.stringify({ name }),
    }),
  revokeToken: (id: string) =>
    request<void>(`/api/v1/tokens/${id}`, { method: 'DELETE' }),

  listResults: (runId: string) =>
    request<TestResult[]>(`/api/v1/runs/${runId}/results`),
  ingestRun: (
    runId: string,
    body: {
      format: string
      content: string
      async_mode?: boolean
      source?: string
    },
  ) =>
    request<{
      inserted: number
      updated: number
      unmatched_case_keys: string[]
      job_id: string | null
      parsed: number
    }>(`/api/v1/runs/${runId}/ingest`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  listCiConfigs: (projectId: string) =>
    request<CiConfig[]>(`/api/v1/projects/${projectId}/ci-configs`),
  createCiConfig: (
    projectId: string,
    body: {
      provider: string
      name: string
      config: Record<string, unknown>
      webhook_secret?: string
    },
  ) =>
    request<CiConfig>(`/api/v1/projects/${projectId}/ci-configs`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  dispatchCi: (
    configId: string,
    body?: { inputs?: Record<string, unknown>; run_name?: string },
  ) =>
    request<CiDispatch>(`/api/v1/ci-configs/${configId}/dispatch`, {
      method: 'POST',
      body: JSON.stringify(body ?? {}),
    }),
  listDispatches: (projectId: string) =>
    request<CiDispatch[]>(`/api/v1/projects/${projectId}/ci-dispatches`),

  getDashboard: (projectId: string) =>
    request<ProjectDashboard>(
      `/api/v1/projects/${projectId}/reports/dashboard`,
    ),
  getRunSummary: (projectId: string, runId: string) =>
    request<RunSummaryReport>(
      `/api/v1/projects/${projectId}/reports/runs/${runId}/summary`,
    ),
  getTrends: (projectId: string, granularity: 'run' | 'day' = 'run', limit = 20) =>
    request<PassFailTrends>(
      `/api/v1/projects/${projectId}/reports/trends?granularity=${granularity}&limit=${limit}`,
    ),
  getCoverage: (projectId: string) =>
    request<RequirementCoverageReport>(
      `/api/v1/projects/${projectId}/reports/coverage`,
    ),
  getFlaky: (projectId: string, lookbackRuns = 20, limit = 50) =>
    request<FlakyReport>(
      `/api/v1/projects/${projectId}/reports/flaky?lookback_runs=${lookbackRuns}&limit=${limit}`,
    ),
  downloadReportExport: async (
    projectId: string,
    opts: { format: 'csv' | 'pdf'; report?: string; runId?: string },
  ) => {
    const qs = new URLSearchParams({
      format: opts.format,
      report: opts.report ?? 'dashboard',
    })
    if (opts.runId) qs.set('run_id', opts.runId)
    const token = getStoredToken()
    const headers: Record<string, string> = {}
    if (token) headers.Authorization = `Bearer ${token}`
    const res = await fetch(
      `${API_BASE}/api/v1/projects/${projectId}/reports/export?${qs}`,
      { headers, credentials: 'include' },
    )
    if (!res.ok) {
      throw new Error((await res.text()) || res.statusText)
    }
    const blob = await res.blob()
    const disposition = res.headers.get('content-disposition') ?? ''
    const match = /filename="?([^"]+)"?/.exec(disposition)
    const filename =
      match?.[1] ??
      `vulyk-report.${opts.format === 'pdf' ? 'pdf' : 'csv'}`
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = filename
    a.click()
    URL.revokeObjectURL(url)
  },
}
