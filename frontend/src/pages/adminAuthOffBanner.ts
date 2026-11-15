import type { AuthStatus, UserPublic } from '../api'

/** System admins see a warning when the instance runs with auth disabled. */
export function shouldShowAuthOffBanner(
  user: UserPublic | null | undefined,
  status: AuthStatus | null | undefined,
): boolean {
  return Boolean(status && !status.auth_required && user?.is_system_admin)
}
