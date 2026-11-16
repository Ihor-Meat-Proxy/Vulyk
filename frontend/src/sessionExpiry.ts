import { setStoredToken } from './token'

export const SESSION_EXPIRED_NOTICE_KEY = 'vulyk.sessionExpiredNotice'
export const RETURN_TO_KEY = 'vulyk.returnTo'
const API_CACHE_NAME = 'vulyk-api-readonly'

let handlingUnauthorized = false

/** Clear bearer token + API SW cache so a stale JWT cannot keep poisoning requests. */
export async function clearClientAuthState(): Promise<void> {
  setStoredToken(null)
  if (typeof caches === 'undefined') return
  try {
    await caches.delete(API_CACHE_NAME)
  } catch {
    /* ignore */
  }
}

export function consumeSessionExpiredNotice(): boolean {
  if (typeof sessionStorage === 'undefined') return false
  const flagged = sessionStorage.getItem(SESSION_EXPIRED_NOTICE_KEY) === '1'
  if (flagged) sessionStorage.removeItem(SESSION_EXPIRED_NOTICE_KEY)
  return flagged
}

export function peekReturnToPath(): string | null {
  if (typeof sessionStorage === 'undefined') return null
  return sessionStorage.getItem(RETURN_TO_KEY)
}

export function consumeReturnToPath(): string | null {
  const path = peekReturnToPath()
  if (path) sessionStorage.removeItem(RETURN_TO_KEY)
  return path
}

function safeReturnTo(pathname: string, search: string): string | null {
  const raw = `${pathname}${search}`
  if (!raw.startsWith('/') || raw.startsWith('//')) return null
  if (raw.startsWith('/login') || raw.startsWith('/setup')) return null
  return raw
}

/**
 * Clear local auth and send the browser to /login with a short notice.
 * Idempotent across concurrent 401s.
 */
export function handleUnauthorizedRedirect(opts?: {
  returnTo?: string | null
  skipRedirect?: boolean
}): void {
  if (handlingUnauthorized) return
  handlingUnauthorized = true

  const returnTo =
    opts?.returnTo ??
    (typeof window !== 'undefined'
      ? safeReturnTo(window.location.pathname, window.location.search)
      : null)

  // Drop the bearer immediately so concurrent requests cannot keep using it.
  setStoredToken(null)
  if (typeof sessionStorage !== 'undefined') {
    sessionStorage.setItem(SESSION_EXPIRED_NOTICE_KEY, '1')
    if (returnTo) sessionStorage.setItem(RETURN_TO_KEY, returnTo)
    else sessionStorage.removeItem(RETURN_TO_KEY)
  }

  // Cache cleanup is best-effort; do not delay navigation on it.
  void clearClientAuthState()

  if (!opts?.skipRedirect && typeof window !== 'undefined') {
    const q = returnTo ? `?returnTo=${encodeURIComponent(returnTo)}` : ''
    if (!window.location.pathname.startsWith('/login')) {
      window.location.assign(`/login${q}`)
      return
    }
  }
  handlingUnauthorized = false
}

/** Test helper to reset the in-flight guard. */
export function resetUnauthorizedGuardForTests(): void {
  handlingUnauthorized = false
}
