/**
 * Format an ISO timestamp with Intl, preferring user timezone, then instance
 * default, then the browser zone.
 */

/** Prefer modern IANA aliases (Ukraine renamed Kiev → Kyiv). */
const TIMEZONE_ALIASES: Record<string, string> = {
  'Europe/Kiev': 'Europe/Kyiv',
}

export function normalizeTimeZone(
  timeZone: string | null | undefined,
): string | undefined {
  if (timeZone == null) return undefined
  const trimmed = timeZone.trim()
  if (!trimmed) return undefined
  return TIMEZONE_ALIASES[trimmed] ?? trimmed
}

export function formatTimestamp(
  value: string | Date | null | undefined,
  opts?: {
    timeZone?: string | null
    instanceTimeZone?: string | null
    locale?: string | null
    dateStyle?: Intl.DateTimeFormatOptions['dateStyle']
    timeStyle?: Intl.DateTimeFormatOptions['timeStyle']
  },
): string {
  if (value == null || value === '') return '—'
  const date = value instanceof Date ? value : new Date(value)
  if (Number.isNaN(date.getTime())) return '—'

  const timeZone =
    normalizeTimeZone(opts?.timeZone) ||
    normalizeTimeZone(opts?.instanceTimeZone) ||
    normalizeTimeZone(Intl.DateTimeFormat().resolvedOptions().timeZone) ||
    'UTC'

  try {
    return new Intl.DateTimeFormat(opts?.locale || undefined, {
      dateStyle: opts?.dateStyle ?? 'medium',
      timeStyle: opts?.timeStyle ?? 'short',
      timeZone,
    }).format(date)
  } catch {
    return date.toLocaleString()
  }
}

export function browserTimeZone(): string {
  return (
    normalizeTimeZone(Intl.DateTimeFormat().resolvedOptions().timeZone) || 'UTC'
  )
}
