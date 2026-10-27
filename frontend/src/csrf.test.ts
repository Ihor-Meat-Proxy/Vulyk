import { describe, expect, it } from 'vitest'

import { csrfTokenFromCookieHeader } from './api'

describe('csrfTokenFromCookieHeader', () => {
  it('returns null when cookie missing', () => {
    expect(csrfTokenFromCookieHeader('')).toBeNull()
    expect(csrfTokenFromCookieHeader('other=1')).toBeNull()
  })

  it('reads vulyk_csrf cookie among others', () => {
    expect(
      csrfTokenFromCookieHeader('a=1; vulyk_csrf=abc%2F123; b=2'),
    ).toBe('abc/123')
  })
})
