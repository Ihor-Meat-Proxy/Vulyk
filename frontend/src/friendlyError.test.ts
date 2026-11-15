import { describe, expect, it } from 'vitest'
import { ApiError } from './api'
import { friendlyErrorMessage } from './friendlyError'

const en: Record<string, string> = {
  'login.genericError': 'login-generic',
  'login.invalidCredentials': 'login-bad',
  'login.rateLimited': 'login-429',
  'notFound.resource': 'not-found',
  'common.unexpectedError': 'unexpected',
  'errors.badRequest': 'bad-request',
  'errors.conflict': 'conflict',
  'errors.forbidden': 'forbidden',
  'errors.unauthorized': 'unauthorized',
  'errors.rateLimited': 'rate-limited',
  'errors.caseDeleted': 'case-deleted',
}

const t = (key: string) => en[key] ?? key

describe('friendlyErrorMessage', () => {
  it('maps stable API codes to translated keys', () => {
    expect(
      friendlyErrorMessage(new ApiError('bad request: x', 400, 'bad_request'), t),
    ).toBe('bad-request')
    expect(
      friendlyErrorMessage(new ApiError('conflict: y', 409, 'conflict'), t),
    ).toBe('conflict')
    expect(
      friendlyErrorMessage(new ApiError('forbidden: z', 403, 'forbidden'), t),
    ).toBe('forbidden')
    expect(
      friendlyErrorMessage(new ApiError('gone: case', 410, 'case_deleted'), t),
    ).toBe('case-deleted')
    expect(
      friendlyErrorMessage(
        new ApiError('too many', 429, 'too_many_requests'),
        t,
      ),
    ).toBe('rate-limited')
    expect(
      friendlyErrorMessage(
        new ApiError('git rate', 429, 'git_host_rate_limit'),
        t,
      ),
    ).toBe('rate-limited')
  })

  it('never returns raw English API messages', () => {
    const msg = friendlyErrorMessage(
      new ApiError('bad request: preferred key already exists', 400, 'bad_request'),
      t,
    )
    expect(msg).toBe('bad-request')
    expect(msg).not.toMatch(/preferred key/i)
  })

  it('uses login-specific copy when kind is login', () => {
    expect(
      friendlyErrorMessage(
        new ApiError('unauthorized: no', 401, 'unauthorized'),
        t,
        'login',
      ),
    ).toBe('login-bad')
    expect(
      friendlyErrorMessage(
        new ApiError('too many', 429, 'too_many_requests'),
        t,
        'login',
      ),
    ).toBe('login-429')
  })
})
