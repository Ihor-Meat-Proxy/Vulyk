export const API_URL = process.env.VULYK_API_URL ?? 'http://127.0.0.1:8080'

export const ADMIN_EMAIL =
  process.env.VULYK_ADMIN_EMAIL ??
  process.env.E2E_EMAIL ??
  'admin@vulyk.local'

export const ADMIN_PASSWORD =
  process.env.VULYK_ADMIN_PASSWORD ??
  process.env.E2E_PASSWORD ??
  'admin12345678901'

export const DEMO_EMAIL =
  process.env.VULYK_DEMO_EMAIL ?? 'user@vulyk.local'

export const DEMO_PASSWORD =
  process.env.VULYK_DEMO_PASSWORD ?? 'user123456789012'

export const OIDC_ISSUER =
  process.env.E2E_OIDC_ISSUER ??
  `http://127.0.0.1:${process.env.E2E_OIDC_PORT ?? 9090}`

export const OIDC_CLIENT_ID = process.env.E2E_OIDC_CLIENT_ID ?? 'vulyk-e2e'
export const OIDC_CLIENT_SECRET =
  process.env.E2E_OIDC_CLIENT_SECRET ?? 'vulyk-e2e-secret'

export function uniqueKey(prefix: string): string {
  const stamp = Date.now().toString(36).toUpperCase().slice(-6)
  const rand = Math.random().toString(36).slice(2, 5).toUpperCase()
  return `${prefix}${stamp}${rand}`.slice(0, 32)
}
