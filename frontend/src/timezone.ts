/**
 * Format an ISO timestamp with Intl, preferring user timezone, then instance
 * default, then the browser zone.
 */
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
    opts?.timeZone ||
    opts?.instanceTimeZone ||
    Intl.DateTimeFormat().resolvedOptions().timeZone

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
  return Intl.DateTimeFormat().resolvedOptions().timeZone
}
