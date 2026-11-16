/** @vitest-environment jsdom */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  clearClientAuthState,
  consumeReturnToPath,
  consumeSessionExpiredNotice,
  handleUnauthorizedRedirect,
  resetUnauthorizedGuardForTests,
  shouldTreatAuthStatusAsExpired,
  RETURN_TO_KEY,
  SESSION_EXPIRED_NOTICE_KEY,
} from './sessionExpiry'

describe('sessionExpiry', () => {
  beforeEach(() => {
    resetUnauthorizedGuardForTests()
    localStorage.clear()
    sessionStorage.clear()
    vi.stubGlobal('caches', {
      delete: vi.fn(async () => true),
    })
  })

  afterEach(() => {
    resetUnauthorizedGuardForTests()
    vi.unstubAllGlobals()
  })

  it('clearClientAuthState removes the bearer token and API cache', async () => {
    localStorage.setItem('vulyk.authToken', 'stale-jwt')
    await clearClientAuthState()
    expect(localStorage.getItem('vulyk.authToken')).toBeNull()
    expect(caches.delete).toHaveBeenCalledWith('vulyk-api-readonly')
  })

  it('handleUnauthorizedRedirect flags notice, returnTo, and navigates to login', () => {
    localStorage.setItem('vulyk.authToken', 'stale-jwt')
    const assign = vi.fn()
    vi.stubGlobal('location', {
      pathname: '/cases/abc',
      search: '?x=1',
      assign,
    })

    handleUnauthorizedRedirect()

    expect(sessionStorage.getItem(SESSION_EXPIRED_NOTICE_KEY)).toBe('1')
    expect(sessionStorage.getItem(RETURN_TO_KEY)).toBe('/cases/abc?x=1')
    expect(localStorage.getItem('vulyk.authToken')).toBeNull()
    expect(assign).toHaveBeenCalledWith(
      '/login?returnTo=%2Fcases%2Fabc%3Fx%3D1',
    )
  })

  it('shouldTreatAuthStatusAsExpired only when a stored token went stale', () => {
    expect(
      shouldTreatAuthStatusAsExpired(true, {
        auth_required: true,
        authenticated: false,
      }),
    ).toBe(true)
    expect(
      shouldTreatAuthStatusAsExpired(false, {
        auth_required: true,
        authenticated: false,
      }),
    ).toBe(false)
    expect(
      shouldTreatAuthStatusAsExpired(true, {
        auth_required: true,
        authenticated: true,
      }),
    ).toBe(false)
    expect(
      shouldTreatAuthStatusAsExpired(true, {
        auth_required: false,
        authenticated: false,
      }),
    ).toBe(false)
  })

  it('consumeSessionExpiredNotice is one-shot', () => {
    sessionStorage.setItem(SESSION_EXPIRED_NOTICE_KEY, '1')
    expect(consumeSessionExpiredNotice()).toBe(true)
    expect(consumeSessionExpiredNotice()).toBe(false)
  })

  it('consumeReturnToPath is one-shot', () => {
    sessionStorage.setItem(RETURN_TO_KEY, '/runs')
    expect(consumeReturnToPath()).toBe('/runs')
    expect(consumeReturnToPath()).toBeNull()
  })
})
