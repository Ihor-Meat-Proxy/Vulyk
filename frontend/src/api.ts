const API_BASE = import.meta.env.VITE_API_URL ?? ''

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    headers: {
      'Content-Type': 'application/json',
      ...(init?.headers ?? {}),
    },
    ...init,
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

export type TestCaseDetail = {
  case: TestCase
  steps: TestStep[]
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
  listProjects: () => request<Project[]>('/api/v1/projects'),
  createProject: (body: { key: string; name: string; description?: string }) =>
    request<Project>('/api/v1/projects', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  listSuites: (projectId: string) =>
    request<Suite[]>(`/api/v1/projects/${projectId}/suites`),
  listCases: (projectId: string, suiteId?: string | null) => {
    const qs = suiteId ? `?suite_id=${suiteId}` : ''
    return request<TestCase[]>(`/api/v1/projects/${projectId}/cases${qs}`)
  },
  getCase: (projectId: string, caseId: string) =>
    request<TestCaseDetail>(`/api/v1/projects/${projectId}/cases/${caseId}`),

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
}
