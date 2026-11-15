import { ApiError } from './api'

export type FriendlyErrorKind = 'login' | 'generic' | 'notFound'

/**
 * Map API errors to translated UI copy via stable `error.code` values.
 * English `error.message` stays on the wire for API clients / logs — the UI
 * must not surface those strings to end users.
 */
export function friendlyErrorMessage(
  err: unknown,
  t: (key: string, vars?: Record<string, string | number>) => string,
  kind: FriendlyErrorKind = 'generic',
): string {
  if (err instanceof ApiError) {
    const code = (err.code || '').toLowerCase()

    if (code === 'not_found' || err.status === 404) {
      return kind === 'login' ? t('login.genericError') : t('notFound.resource')
    }
    if (code === 'case_deleted' || err.status === 410) {
      return t('errors.caseDeleted')
    }
    if (code === 'unauthorized' || err.status === 401) {
      return kind === 'login'
        ? t('login.invalidCredentials')
        : t('errors.unauthorized')
    }
    if (code === 'forbidden' || err.status === 403) {
      return t('errors.forbidden')
    }
    if (
      code === 'too_many_requests' ||
      code === 'rate_limited' ||
      code === 'git_host_rate_limit' ||
      err.status === 429
    ) {
      return kind === 'login' ? t('login.rateLimited') : t('errors.rateLimited')
    }
    if (code === 'conflict' || err.status === 409) {
      return t('errors.conflict')
    }
    if (code === 'bad_request' || err.status === 400) {
      return kind === 'login'
        ? t('login.genericError')
        : t('errors.badRequest')
    }
    if (code === 'internal' || err.status >= 500) {
      return t('common.unexpectedError')
    }

    // Unknown code: never echo English API message into the UI.
    return kind === 'login'
      ? t('login.genericError')
      : t('common.unexpectedError')
  }

  return kind === 'login'
    ? t('login.genericError')
    : t('common.unexpectedError')
}
