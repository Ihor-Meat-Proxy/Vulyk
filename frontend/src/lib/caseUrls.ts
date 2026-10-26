/** UUID v1–v5 path segment (case-insensitive). */
const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

/** Human-readable case key PREFIX-N (case-insensitive). */
const CASE_KEY_RE = /^[A-Z][A-Z0-9]{1,7}-\d+$/i

export function isCaseUuid(value: string | undefined | null): boolean {
  return Boolean(value && UUID_RE.test(value.trim()))
}

export function isCaseKey(value: string | undefined | null): boolean {
  return Boolean(value && CASE_KEY_RE.test(value.trim()))
}

/** Canonical UI path for a case detail page (prefer key). */
export function casePath(keyOrId: string): string {
  return `/cases/${encodeURIComponent(keyOrId)}`
}

export function normalizeCaseKey(key: string): string {
  return key.trim().toUpperCase()
}
