import type { ProjectRole } from './api'

/** Fine-grained capabilities mirrored from backend `Permission`. */
export type Permission =
  | 'manage_users'
  | 'manage_system_settings'
  | 'manage_sso'
  | 'manage_global_integrations'
  | 'view_project'
  | 'update_project'
  | 'delete_project'
  | 'manage_memberships'
  | 'manage_suites'
  | 'create_edit_cases'
  | 'delete_cases'
  | 'manage_requirements'
  | 'manage_plans'
  | 'import_cases'
  | 'view_reports'
  | 'export_reports'
  | 'execute_runs'
  | 'upload_results'
  | 'manage_attachments'
  | 'comment'
  | 'trigger_ci'
  | 'manage_ci_config'
  | 'llm_assist'
  | 'manage_llm_config'
  | 'manage_api_tokens'

const SYSTEM_ONLY: ReadonlySet<Permission> = new Set([
  'manage_users',
  'manage_system_settings',
  'manage_sso',
  'manage_global_integrations',
])

const ROLE_PERMISSIONS: Record<ProjectRole, readonly Permission[]> = {
  admin: [
    'view_project',
    'update_project',
    'delete_project',
    'manage_memberships',
    'manage_suites',
    'create_edit_cases',
    'delete_cases',
    'manage_requirements',
    'manage_plans',
    'import_cases',
    'view_reports',
    'export_reports',
    'execute_runs',
    'upload_results',
    'manage_attachments',
    'comment',
    'trigger_ci',
    'manage_ci_config',
    'llm_assist',
    'manage_llm_config',
    'manage_api_tokens',
  ],
  test_architect: [
    'view_project',
    'update_project',
    'manage_suites',
    'create_edit_cases',
    'delete_cases',
    'manage_requirements',
    'manage_plans',
    'import_cases',
    'view_reports',
    'export_reports',
    'execute_runs',
    'upload_results',
    'manage_attachments',
    'comment',
    'llm_assist',
    'manage_llm_config',
  ],
  test_automation_engineer: [
    'view_project',
    'manage_suites',
    'create_edit_cases',
    'view_reports',
    'export_reports',
    'execute_runs',
    'upload_results',
    'manage_attachments',
    'comment',
    'trigger_ci',
    'manage_ci_config',
    'llm_assist',
    'manage_api_tokens',
  ],
  developer: [
    'view_project',
    'view_reports',
    'export_reports',
    'execute_runs',
    'upload_results',
    'manage_attachments',
    'comment',
    'trigger_ci',
  ],
  viewer: ['view_project', 'view_reports', 'export_reports'],
}

export type PermissionActor = {
  role?: ProjectRole | null
  isAdmin?: boolean
  isSystemAdmin?: boolean
}

export function isSystemOnlyPermission(permission: Permission): boolean {
  return SYSTEM_ONLY.has(permission)
}

/**
 * Whether `actor` may perform `permission`.
 * System admin / project admin (via `isAdmin` or role `admin`) get project perms;
 * system-only perms require `isSystemAdmin`.
 * When actor is null/undefined (open mode), allow non-system permissions.
 */
export function can(
  actor: PermissionActor | null | undefined,
  permission: Permission,
): boolean {
  if (isSystemOnlyPermission(permission)) {
    return Boolean(actor?.isSystemAdmin)
  }
  if (!actor) return true
  if (actor.isSystemAdmin || actor.isAdmin) return true
  if (actor.role === 'admin') return true
  if (!actor.role) return false
  return ROLE_PERMISSIONS[actor.role].includes(permission)
}

export function roleDisplayName(role: ProjectRole): string {
  switch (role) {
    case 'admin':
      return 'Admin'
    case 'test_architect':
      return 'Test Architect'
    case 'test_automation_engineer':
      return 'Test Automation Engineer'
    case 'developer':
      return 'Developer'
    case 'viewer':
      return 'Viewer'
  }
}

export const PROJECT_ROLES: ProjectRole[] = [
  'admin',
  'test_architect',
  'test_automation_engineer',
  'developer',
  'viewer',
]
