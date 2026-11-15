import { describe, expect, it } from 'vitest'
import type { AuthStatus, UserPublic } from '../api'
import { shouldShowAuthOffBanner } from './adminAuthOffBanner'

const admin: UserPublic = {
  id: 'admin-1',
  email: 'admin@example.com',
  display_name: 'Admin',
  is_system_admin: true,
  must_change_password: false,
  locale: 'en',
  timezone: 'UTC',
  preferences: {
    theme: 'system',
    font_size: 14,
    density: 'comfortable',
    reduced_motion: null,
  },
  disabled_at: null,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
}

function status(authRequired: boolean): AuthStatus {
  return {
    auth_required: authRequired,
    bootstrap_available: false,
    setup_required: false,
    registration_mode: 'disabled',
    default_locale: 'en',
    default_timezone: 'UTC',
    default_theme: 'system',
    instance_name: 'Vulyk',
    authenticated: true,
    user: admin,
    sso_providers: [],
    password_policy: {
      min_length: 16,
      require_letter: true,
      require_digit: true,
    },
  }
}

describe('shouldShowAuthOffBanner', () => {
  it('shows for system admin when auth is off', () => {
    expect(shouldShowAuthOffBanner(admin, status(false))).toBe(true)
  })

  it('hides when auth is required', () => {
    expect(shouldShowAuthOffBanner(admin, status(true))).toBe(false)
  })

  it('hides for non-admin even when auth is off', () => {
    const member = { ...admin, is_system_admin: false }
    expect(shouldShowAuthOffBanner(member, status(false))).toBe(false)
  })

  it('hides when status or user is missing', () => {
    expect(shouldShowAuthOffBanner(null, status(false))).toBe(false)
    expect(shouldShowAuthOffBanner(admin, null)).toBe(false)
  })
})
