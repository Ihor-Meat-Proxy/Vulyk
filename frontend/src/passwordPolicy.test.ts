import { describe, expect, it } from 'vitest'
import {
  DEFAULT_PASSWORD_POLICY,
  passwordFieldMinLength,
  passwordMeetsPolicy,
  passwordRequirementFlags,
} from './passwordPolicy'

describe('passwordPolicy', () => {
  it('rejects short and incomplete passwords at default 16', () => {
    expect(passwordMeetsPolicy('admin')).toBe(false)
    expect(passwordMeetsPolicy('shortpass12345')).toBe(false) // 14
    expect(passwordMeetsPolicy('longenoughpass12')).toBe(true) // 16
    expect(passwordMeetsPolicy('abcdefghijklmnop')).toBe(false)
    expect(passwordMeetsPolicy('1234567890123456')).toBe(false)
  })

  it('login never sets HTML minLength; set-password does', () => {
    expect(passwordFieldMinLength(false)).toBeUndefined()
    expect(passwordFieldMinLength(true)).toBe(DEFAULT_PASSWORD_POLICY.min_length)
  })

  it('exposes live requirement flags', () => {
    const flags = passwordRequirementFlags('ab1', { min_length: 16, require_letter: true, require_digit: true })
    expect(flags).toEqual({ length: false, letter: true, digit: true })
  })
})
