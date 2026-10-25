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
}
