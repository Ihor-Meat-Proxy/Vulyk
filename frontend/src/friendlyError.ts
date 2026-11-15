import { ApiError } from './api'

/** Map raw API errors to short, user-facing copy (login / not-found / generic). */
export function friendlyErrorMessage(
  err: unknown,
  t: (key: string, vars?: Record<string, string | number>) => string,
  kind: 'login' | 'generic' | 'notFound' = 'generic',
): string {
  if (err instanceof ApiError) {
    if (err.status === 404 || err.code === 'not_found') {
      return kind === 'login' ? t('login.genericError') : t('notFound.resource')
    }
    if (err.status === 401 || err.code === 'unauthorized') {
      return t('login.invalidCredentials')
    }
    if (err.status === 429 || err.code === 'rate_limited') {
      return t('login.rateLimited')
    }
    const raw = err.message.trim()
    if (
      /^bad request:/i.test(raw) ||
      /^not found:/i.test(raw) ||
      /^internal/i.test(raw) ||
      /^forbidden:/i.test(raw)
    ) {
      return kind === 'login'
        ? t('login.genericError')
        : t('common.unexpectedError')
    }
    // Prefer short API messages that are already human-readable.
    if (raw && raw.length < 120 && !raw.includes('{')) {
      return raw
    }
  }
  if (err instanceof Error && err.message && err.message.length < 120) {
    const raw = err.message.trim()
    if (/^bad request:|^not found:|^internal/i.test(raw)) {
      return kind === 'login'
        ? t('login.genericError')
        : t('common.unexpectedError')
    }
  }
  return kind === 'login'
    ? t('login.genericError')
    : t('common.unexpectedError')
}
