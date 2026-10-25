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
})
