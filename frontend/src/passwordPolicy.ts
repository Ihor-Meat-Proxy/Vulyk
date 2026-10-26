/** Mirrors backend `PasswordPolicyPublic` from `/api/v1/auth/status`. */

export type PasswordPolicy = {
  min_length: number
  require_letter: boolean
  require_digit: boolean
}

export const DEFAULT_PASSWORD_POLICY: PasswordPolicy = {
  min_length: 16,
  require_letter: true,
  require_digit: true,
}

export function passwordMeetsPolicy(
  password: string,
  policy: PasswordPolicy = DEFAULT_PASSWORD_POLICY,
): boolean {
  if ([...password].length < policy.min_length) return false
  if (policy.require_letter && !/[a-zA-Z]/.test(password)) return false
  if (policy.require_digit && !/\d/.test(password)) return false
  return true
}

/** HTML minLength only when setting a password — never on login. */
export function passwordFieldMinLength(
  forSetPassword: boolean,
  policy: PasswordPolicy = DEFAULT_PASSWORD_POLICY,
): number | undefined {
  return forSetPassword ? policy.min_length : undefined
}

export function passwordRequirementFlags(
  password: string,
  policy: PasswordPolicy = DEFAULT_PASSWORD_POLICY,
): { length: boolean; letter: boolean; digit: boolean } {
  return {
    length: [...password].length >= policy.min_length,
    letter: !policy.require_letter || /[a-zA-Z]/.test(password),
    digit: !policy.require_digit || /\d/.test(password),
  }
}
