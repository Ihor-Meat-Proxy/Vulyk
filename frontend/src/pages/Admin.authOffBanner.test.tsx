/** @vitest-environment jsdom */
import { act } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { AuthStatus, UserPublic } from '../api'
import { TestIds } from '../testing'

const useAuthMock = vi.fn()

vi.mock('../auth', () => ({
  useAuth: () => useAuthMock(),
}))

vi.mock('../i18n', () => ({
  useI18n: () => ({
    locale: 'en',
    t: (key: string) => key,
    setLocale: vi.fn(),
    formatNumber: (n: number) => String(n),
    formatDate: (d: Date | string) => String(d),
    intl: null,
  }),
}))

vi.mock('../hooks/useProjectPermissions', () => ({
  useProjectPermissions: () => ({
    role: null,
    actor: null,
    can: () => false,
    loading: false,
  }),
}))

vi.mock('../api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../api')>()
  return {
    ...actual,
    api: {
      ...actual.api,
      listUsers: vi.fn().mockResolvedValue([]),
      listAdminIntegrations: vi.fn().mockResolvedValue([]),
      getAdminSettings: vi.fn().mockResolvedValue(null),
      listSsoProvidersAdmin: vi.fn().mockResolvedValue([]),
      listMemberships: vi.fn().mockResolvedValue([]),
      listTokens: vi.fn().mockResolvedValue([]),
      listLlmProviders: vi.fn().mockResolvedValue([]),
      listLlmConfigs: vi.fn().mockResolvedValue([]),
    },
  }
})

import { Admin } from './Admin'

const adminUser: UserPublic = {
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
    user: adminUser,
    sso_providers: [],
    password_policy: {
      min_length: 16,
      require_letter: true,
      require_digit: true,
    },
  }
}

describe('Admin auth-off banner', () => {
  let container: HTMLDivElement
  let root: Root

  beforeEach(() => {
    container = document.createElement('div')
    document.body.appendChild(container)
    root = createRoot(container)
    useAuthMock.mockReset()
  })

  afterEach(() => {
    act(() => {
      root.unmount()
    })
    container.remove()
  })

  it('shows adminAuthOffBanner when auth_required is false for system admin', async () => {
    useAuthMock.mockReturnValue({
      user: adminUser,
      status: status(false),
    })

    await act(async () => {
      root.render(
        <Admin projects={[]} project={null} onProjectsChanged={() => {}} />,
      )
    })
    await act(async () => {
      await Promise.resolve()
    })

    const banner = container.querySelector(
      `[data-testid="${TestIds.adminAuthOffBanner}"]`,
    )
    expect(banner).not.toBeNull()
    expect(banner?.textContent).toBe('admin.authOffWarning')
  })

  it('hides adminAuthOffBanner when auth_required is true', async () => {
    useAuthMock.mockReturnValue({
      user: adminUser,
      status: status(true),
    })

    await act(async () => {
      root.render(
        <Admin projects={[]} project={null} onProjectsChanged={() => {}} />,
      )
    })
    await act(async () => {
      await Promise.resolve()
    })

    expect(
      container.querySelector(`[data-testid="${TestIds.adminAuthOffBanner}"]`),
    ).toBeNull()
  })

  it('hides adminAuthOffBanner for non-system-admin even when auth is off', async () => {
    const member: UserPublic = { ...adminUser, is_system_admin: false }
    useAuthMock.mockReturnValue({
      user: member,
      status: { ...status(false), user: member },
    })

    await act(async () => {
      root.render(
        <Admin projects={[]} project={null} onProjectsChanged={() => {}} />,
      )
    })
    await act(async () => {
      await Promise.resolve()
    })

    expect(
      container.querySelector(`[data-testid="${TestIds.adminAuthOffBanner}"]`),
    ).toBeNull()
  })
})
