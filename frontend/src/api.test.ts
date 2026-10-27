/** @vitest-environment jsdom */
import { afterEach, describe, expect, it, vi } from 'vitest'
import { api, isAbortError, ssoStartUrl } from './api'

describe('api client', () => {
  const originalFetch = globalThis.fetch

  afterEach(() => {
    globalThis.fetch = originalFetch
  })

  it('exposes listProjects', () => {
    expect(typeof api.listProjects).toBe('function')
  })

  it('passes AbortSignal through listCases / getCase / getRun', async () => {
    const ac = new AbortController()
    const fetchMock = vi.fn(async (_url: RequestInfo | URL, init?: RequestInit) => {
      expect(init?.signal).toBe(ac.signal)
      return new Response(JSON.stringify({ items: [], total: 0, limit: 1, offset: 0 }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    })
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
})
