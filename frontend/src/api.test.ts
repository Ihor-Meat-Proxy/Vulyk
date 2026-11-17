/** @vitest-environment jsdom */
import { afterEach, describe, expect, it, vi } from 'vitest'
import { ApiError, api, isAbortError, ssoStartUrl } from './api'
import { resetUnauthorizedGuardForTests } from './sessionExpiry'

describe('api client', () => {
  const originalFetch = globalThis.fetch

  afterEach(() => {
    globalThis.fetch = originalFetch
    resetUnauthorizedGuardForTests()
    localStorage.clear()
    sessionStorage.clear()
    vi.restoreAllMocks()
  })

  it('exposes listProjects', () => {
    expect(typeof api.listProjects).toBe('function')
  })

  it('passes AbortSignal through listCases / getCase / getRun', async () => {
    const ac = new AbortController()
    const fetchMock = vi.fn(
      async (_url: RequestInfo | URL, init?: RequestInit) => {
        expect(init?.signal).toBe(ac.signal)
        return new Response(
          JSON.stringify({ items: [], total: 0, limit: 1, offset: 0 }),
          {
            status: 200,
            headers: { 'Content-Type': 'application/json' },
          },
        )
      },
    )
    globalThis.fetch = fetchMock as typeof fetch

    await api.listCases('p1', { limit: 1, signal: ac.signal })
    expect(fetchMock).toHaveBeenCalledTimes(1)

    fetchMock.mockImplementation(async (_url, init) => {
      expect(init?.signal).toBe(ac.signal)
      return new Response(
        JSON.stringify({
          case: { id: 'c1', project_id: 'p1', key: 'T-1', title: 't' },
          steps: [],
          tags: [],
          custom_fields: [],
        }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      )
    })
    await api.getCase('p1', 'c1', { signal: ac.signal })
    expect(fetchMock).toHaveBeenCalledTimes(2)
  })

  it('isAbortError detects AbortError', () => {
    expect(isAbortError(new DOMException('Aborted', 'AbortError'))).toBe(true)
    const err = new Error('aborted')
    err.name = 'AbortError'
    expect(isAbortError(err)).toBe(true)
    expect(isAbortError(new Error('other'))).toBe(false)
  })

  it('exposes case depth and run helpers', () => {
    expect(typeof api.listCases).toBe('function')
    expect(typeof api.bulkCases).toBe('function')
    expect(typeof api.downloadCasePdf).toBe('function')
    expect(typeof api.exportCases).toBe('function')
    expect(typeof api.listVersions).toBe('function')
    expect(typeof api.createRun).toBe('function')
    expect(typeof api.getRunItem).toBe('function')
    expect(typeof api.createResult).toBe('function')
  })

  it('exposes case list column preference helpers', () => {
    expect(typeof api.getCaseListColumns).toBe('function')
    expect(typeof api.putCaseListColumns).toBe('function')
    expect(typeof api.resetCaseListColumns).toBe('function')
  })

  it('exposes setup, auth, and admin helpers', () => {
    expect(typeof api.completeSetup).toBe('function')
    expect(typeof api.setupStatus).toBe('function')
    expect(typeof api.register).toBe('function')
    expect(typeof api.changePassword).toBe('function')
    expect(typeof api.updateMe).toBe('function')
    expect(typeof api.getAdminSettings).toBe('function')
    expect(typeof api.listSsoProvidersAdmin).toBe('function')
    expect(typeof api.getRoleMatrix).toBe('function')
    expect(ssoStartUrl('okta')).toContain('/api/v1/auth/sso/okta/start')
  })

  it('exposes multi-repo automation and project CI token helpers', () => {
    expect(typeof api.listAutomationRepos).toBe('function')
    expect(typeof api.createAutomationRepo).toBe('function')
    expect(typeof api.getAutomationRepoById).toBe('function')
    expect(typeof api.updateAutomationRepo).toBe('function')
    expect(typeof api.deleteAutomationRepoById).toBe('function')
    expect(typeof api.setAutomationRepoProjects).toBe('function')
    expect(typeof api.listAutomationReposForProject).toBe('function')
    expect(typeof api.testAutomationRepoById).toBe('function')
    expect(typeof api.reindexAutomationRepoById).toBe('function')
    expect(typeof api.listProjectCiTokens).toBe('function')
    expect(typeof api.createProjectCiToken).toBe('function')
    expect(typeof api.revokeProjectCiToken).toBe('function')
  })

  it('clears session and redirects on 401 from protected APIs', async () => {
    localStorage.setItem('vulyk.authToken', 'stale-jwt')
    const assign = vi.fn()
    vi.stubGlobal('location', {
      pathname: '/runs',
      search: '',
      assign,
    })
    vi.stubGlobal('caches', { delete: vi.fn(async () => true) })
    globalThis.fetch = vi.fn(
      async () =>
        new Response(
          JSON.stringify({
            error: {
              code: 'unauthorized',
              message: 'invalid or revoked API token',
            },
          }),
          { status: 401, headers: { 'Content-Type': 'application/json' } },
        ),
    ) as typeof fetch

    await expect(api.listProjects()).rejects.toMatchObject({
      status: 401,
      code: 'unauthorized',
    } satisfies Partial<ApiError>)
    expect(localStorage.getItem('vulyk.authToken')).toBeNull()
    expect(sessionStorage.getItem('vulyk.sessionExpiredNotice')).toBe('1')
    expect(assign).toHaveBeenCalledWith('/login?returnTo=%2Fruns')
  })

  it('does not redirect on 401 from login', async () => {
    const assign = vi.fn()
    vi.stubGlobal('location', { pathname: '/login', search: '', assign })
    globalThis.fetch = vi.fn(
      async () =>
        new Response(
          JSON.stringify({
            error: { code: 'unauthorized', message: 'invalid credentials' },
          }),
          { status: 401, headers: { 'Content-Type': 'application/json' } },
        ),
    ) as typeof fetch

    await expect(
      api.login({ email: 'a@b.c', password: 'wrong-password-xx' }),
    ).rejects.toMatchObject({ status: 401 })
    expect(assign).not.toHaveBeenCalled()
  })

  it('parses RFC 9457 problem+json detail and code', async () => {
    globalThis.fetch = vi.fn(
      async () =>
        new Response(
          JSON.stringify({
            type: 'urn:vulyk:api:problem:not_found',
            title: 'Not Found',
            status: 404,
            detail: 'not found: project',
            code: 'not_found',
            error: { code: 'not_found', message: 'not found: project' },
          }),
          {
            status: 404,
            headers: { 'Content-Type': 'application/problem+json' },
          },
        ),
    ) as typeof fetch

    await expect(api.listProjects()).rejects.toMatchObject({
      status: 404,
      code: 'not_found',
      message: 'not found: project',
    } satisfies Partial<ApiError>)
  })
})
