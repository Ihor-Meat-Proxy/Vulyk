import { describe, expect, it } from 'vitest'
import { api, ssoStartUrl } from './api'

describe('api client', () => {
  it('exposes listProjects', () => {
    expect(typeof api.listProjects).toBe('function')
  })

  it('exposes case depth and run helpers', () => {
    expect(typeof api.listCases).toBe('function')
    expect(typeof api.bulkCases).toBe('function')
    expect(typeof api.listVersions).toBe('function')
    expect(typeof api.createRun).toBe('function')
    expect(typeof api.getRunItem).toBe('function')
    expect(typeof api.createResult).toBe('function')
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
