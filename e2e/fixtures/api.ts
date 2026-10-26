import { expect, type APIRequestContext } from '@playwright/test'
import { API_URL } from './env'

export type AuthSession = {
  token: string
  user: {
    id: string
    email: string
    display_name: string
    is_system_admin: boolean
  }
}

function authHeaders(token?: string): Record<string, string> {
  return token ? { Authorization: `Bearer ${token}` } : {}
}

export async function apiJson<T>(
  request: APIRequestContext,
  method: string,
  path: string,
  opts: {
    token?: string
    data?: unknown
    expectedStatus?: number | number[]
  } = {},
): Promise<T> {
  const url = path.startsWith('http') ? path : `${API_URL}${path}`
  const res = await request.fetch(url, {
    method,
    headers: {
      'Content-Type': 'application/json',
      ...authHeaders(opts.token),
    },
    data: opts.data,
  })
  const expected = opts.expectedStatus
  if (expected !== undefined) {
    const list = Array.isArray(expected) ? expected : [expected]
    expect(list, await res.text()).toContain(res.status())
  } else {
    expect(res.ok(), `${method} ${path} → ${res.status()}: ${await res.text()}`).toBeTruthy()
  }
  if (res.status() === 204) return undefined as T
  const text = await res.text()
  if (!text) return undefined as T
  return JSON.parse(text) as T
}

export async function loginApi(
  request: APIRequestContext,
  email: string,
  password: string,
): Promise<AuthSession> {
  const body = await apiJson<{
    token: string
    user: AuthSession['user']
  }>(request, 'POST', '/api/v1/auth/login', {
    data: { email, password },
  })
  return { token: body.token, user: body.user }
}

export async function createProject(
  request: APIRequestContext,
  token: string,
  body: { key?: string; name: string; description?: string },
) {
  return apiJson<{ id: string; key: string; name: string }>(
    request,
    'POST',
    '/api/v1/projects',
    { token, data: body },
  )
}

export async function createSuite(
  request: APIRequestContext,
  token: string,
  projectId: string,
  name: string,
) {
  return apiJson<{ id: string; name: string }>(
    request,
    'POST',
    `/api/v1/projects/${projectId}/suites`,
    { token, data: { name, sort_order: 0 } },
  )
}

export async function createCase(
  request: APIRequestContext,
  token: string,
  projectId: string,
  body: Record<string, unknown>,
) {
  return apiJson<{ case: { id: string; key: string; title: string } }>(
    request,
    'POST',
    `/api/v1/projects/${projectId}/cases`,
    { token, data: body },
  )
}

export async function createUser(
  request: APIRequestContext,
  token: string,
  body: {
    email: string
    password: string
    display_name: string
    is_system_admin?: boolean
  },
) {
  return apiJson<{ id: string; email: string }>(
    request,
    'POST',
    '/api/v1/users',
    { token, data: body },
  )
}

export async function upsertMembership(
  request: APIRequestContext,
  token: string,
  projectId: string,
  userId: string,
  role: string,
) {
  return apiJson(request, 'POST', `/api/v1/projects/${projectId}/memberships`, {
    token,
    data: { user_id: userId, role },
  })
}

export async function createApiToken(
  request: APIRequestContext,
  token: string,
  name: string,
) {
  return apiJson<{ token: { id: string; name: string }; secret: string }>(
    request,
    'POST',
    '/api/v1/tokens',
    { token, data: { name } },
  )
}

export async function createRun(
  request: APIRequestContext,
  token: string,
  projectId: string,
  body: Record<string, unknown>,
) {
  return apiJson<{ run: { id: string; name: string; status: string } }>(
    request,
    'POST',
    `/api/v1/projects/${projectId}/runs`,
    { token, data: body },
  )
}

export async function createPlan(
  request: APIRequestContext,
  token: string,
  projectId: string,
  body: { name: string; description?: string; case_ids?: string[] },
) {
  return apiJson<{ plan: { id: string; name: string }; case_ids: string[] }>(
    request,
    'POST',
    `/api/v1/projects/${projectId}/plans`,
    { token, data: body },
  )
}

export async function listSsoProvidersAdmin(
  request: APIRequestContext,
  token: string,
) {
  return apiJson<
    Array<{
      id: string
      provider_key: string
      display_name: string
      enabled: boolean
      client_id: string
    }>
  >(request, 'GET', '/api/v1/admin/sso/providers', { token })
}

export async function updateSsoProvider(
  request: APIRequestContext,
  token: string,
  id: string,
  body: Record<string, unknown>,
) {
  return apiJson(request, 'PUT', `/api/v1/admin/sso/providers/${id}`, {
    token,
    data: body,
  })
}

export async function updateAdminSettings(
  request: APIRequestContext,
  token: string,
  body: Record<string, unknown>,
) {
  return apiJson(request, 'PUT', '/api/v1/admin/settings', { token, data: body })
}

export async function authStatus(request: APIRequestContext) {
  return apiJson<{
    authenticated: boolean
    auth_required: boolean
    bootstrap_available: boolean
    setup_required: boolean
    registration_mode: string
    sso_providers: Array<{ id: string; provider_key: string; display_name: string; enabled: boolean }>
    default_locale?: string
    default_timezone?: string
  }>(request, 'GET', '/api/v1/auth/status')
}
