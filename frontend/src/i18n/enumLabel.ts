/**
 * Translate a namespaced enum value the same way CaseDetail does:
 * `t('priority.critical')` / `t('status.ready')`, falling back to the raw
 * value when the key is missing from the active (and English) dictionaries.
 *
 * Also retries with a lowercased value so result statuses like `Passed`
 * resolve to `status.passed`.
 */
export function enumLabel(
  t: (key: string) => string,
  namespace: string,
  value: string,
): string {
  const raw = value.trim()
  if (!raw) return value

  const key = `${namespace}.${raw}`
  const label = t(key)
  if (label !== key) return label

  const lower = raw.toLowerCase()
  if (lower !== raw) {
    const lowerKey = `${namespace}.${lower}`
    const lowerLabel = t(lowerKey)
    if (lowerLabel !== lowerKey) return lowerLabel
  }

  return raw
}
