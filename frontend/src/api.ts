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
  created_at: string
  updated_at: string
}

export type Suite = {
  id: string
  project_id: string
  parent_id: string | null
  name: string
  sort_order: number
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
  executed_at: string
}

export type TestResultDetail = {
  result: TestResult
  steps: ResultStep[]
}

export type RunItemExecution = {
  item: RunItemView
  steps: TestStep[]
  latest_result: TestResultDetail | null
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

export type UserPublic = {
  id: string
  email: string
  display_name: string
  is_system_admin: boolean
  disabled_at: string | null
  created_at: string
  updated_at: string
}

export type AuthStatus = {
  auth_required: boolean
  bootstrap_available: boolean
  authenticated: boolean
  user: UserPublic | null
}

export type AuthResponse = {
  token: string
  token_type: string
  expires_in_hours: number
  user: UserPublic
}

export type ProjectRole = 'admin' | 'project_lead' | 'tester' | 'viewer'

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

export type ApiTokenPublic = {
  id: string
  name: string
  token_prefix: string
  created_at: string
  last_used_at: string | null
  revoked_at: string | null
}

export type TestRun = {
  id: string
  project_id: string
  name: string
  status: string
  source: string
  external_ref: string | null
  meta: Record<string, unknown>
  started_at: string | null
  finished_at: string | null
  created_at: string
  updated_at: string
}

export type TestResult = {
  id: string
  run_id: string
  case_id: string | null
  status: string
  message: string
  source: string
  external_id: string | null
  duration_ms: number | null
  executed_at: string
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

export const api = {
  authStatus: () => request<AuthStatus>('/api/v1/auth/status'),
  login: (body: { email: string; password: string }) =>
    request<AuthResponse>('/api/v1/auth/login', {
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
  createProject: (body: { key: string; name: string; description?: string }) =>
    request<Project>('/api/v1/projects', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  listSuites: (projectId: string) =>
    request<Suite[]>(`/api/v1/projects/${projectId}/suites`),
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
    body: { case_ids: string[]; framework: string },
  ) =>
    request<{ provider: string; model: string; framework: string; code: string }>(
      `/api/v1/projects/${projectId}/llm/generate-code`,
      { method: 'POST', body: JSON.stringify(body) },
    ),
  askTests: (projectId: string, question: string) =>
    request<{ provider: string; model: string; answer: string }>(
      `/api/v1/projects/${projectId}/llm/ask`,
      { method: 'POST', body: JSON.stringify({ question }) },
    ),

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
}
