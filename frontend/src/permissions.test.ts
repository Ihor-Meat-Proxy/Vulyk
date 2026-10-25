import { describe, expect, it } from 'vitest'
import { can } from './permissions'

describe('permissions.can', () => {
  it('allows project perms in open mode (null actor)', () => {
    expect(can(null, 'create_edit_cases')).toBe(true)
    expect(can(null, 'manage_users')).toBe(false)
  })

  it('grants system admin everything', () => {
    expect(can({ isSystemAdmin: true }, 'manage_users')).toBe(true)
    expect(can({ isSystemAdmin: true }, 'delete_project')).toBe(true)
  })

  it('mirrors role matrix for viewer / developer / TAE / architect', () => {
    expect(can({ role: 'viewer' }, 'view_reports')).toBe(true)
    expect(can({ role: 'viewer' }, 'execute_runs')).toBe(false)

    expect(can({ role: 'developer' }, 'execute_runs')).toBe(true)
    expect(can({ role: 'developer' }, 'create_edit_cases')).toBe(false)

    expect(can({ role: 'test_automation_engineer' }, 'create_edit_cases')).toBe(
      true,
    )
    expect(can({ role: 'test_automation_engineer' }, 'delete_cases')).toBe(
      false,
    )
    expect(can({ role: 'test_automation_engineer' }, 'manage_plans')).toBe(
      false,
    )
    expect(can({ role: 'test_automation_engineer' }, 'manage_api_tokens')).toBe(
      true,
    )

    expect(can({ role: 'test_architect' }, 'delete_cases')).toBe(true)
    expect(can({ role: 'test_architect' }, 'manage_plans')).toBe(true)
    expect(can({ role: 'test_architect' }, 'delete_project')).toBe(false)
    expect(can({ role: 'test_architect' }, 'manage_users')).toBe(false)
  })

  it('treats project admin as full project access', () => {
    expect(can({ role: 'admin' }, 'delete_project')).toBe(true)
    expect(can({ role: 'admin' }, 'manage_users')).toBe(false)
    expect(can({ isAdmin: true }, 'manage_plans')).toBe(true)
  })
})
