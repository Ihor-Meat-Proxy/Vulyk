import { useCallback, useEffect, useMemo, useState } from 'react'
import { api, type ProjectRole } from '../api'
import { useAuth } from '../auth'
import { can, type Permission, type PermissionActor } from '../permissions'

/**
 * Resolve the current user's project role and expose `can(permission)`.
 * System admins and open mode (no auth) are treated as fully allowed for
 * project permissions.
 */
export function useProjectPermissions(projectId: string | null) {
  const { user, status } = useAuth()
  const [role, setRole] = useState<ProjectRole | null>(null)
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    if (!projectId || !user || user.is_system_admin) {
      setRole(null)
      return
    }
    let cancelled = false
    setLoading(true)
    api
      .listMemberships(projectId)
      .then((rows) => {
        if (cancelled) return
        const mine = rows.find((m) => m.user_id === user.id)
        setRole(mine?.role ?? null)
      })
      .catch(() => {
        if (!cancelled) setRole(null)
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [projectId, user])

  const actor = useMemo<PermissionActor | null>(() => {
    if (!status?.auth_required) return null
    if (!user) return { role: null, isSystemAdmin: false }
    return {
      role,
      isSystemAdmin: user.is_system_admin,
      isAdmin: user.is_system_admin || role === 'admin',
    }
  }, [status?.auth_required, user, role])

  const check = useCallback(
    (permission: Permission) => can(actor, permission),
    [actor],
  )

  return { role, actor, can: check, loading }
}
