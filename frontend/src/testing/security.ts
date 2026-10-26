/**
 * Guards against PII / secrets leaking into data-testid and related data-* attrs.
 * See docs/testing/test-ids.md § Security.
 */

const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

/** Case/project-style keys: VQA-001, KOC-12, VULYK_QA, ci-mcp, en, google */
export const SAFE_KEY_RE = /^[A-Za-z0-9][A-Za-z0-9_./:-]{0,63}$/

/** True when `value` is safe for `data-entity-key` (no spaces, emails, or secrets). */
export function isSafeEntityKey(value: string | null | undefined): value is string {
  return value != null && value !== '' && SAFE_KEY_RE.test(value)
}

/** Static test id: kebab-case */
const TEST_ID_RE = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)+$/

export type SuspiciousReason = {
  attr: string
  value: string
  reason: string
}

/**
 * Return a human reason if `value` looks like PII or a secret for the given attr.
 * Allowed: static kebab test ids, UUIDs, short API keys (case/project/locale/provider), indexes/depths.
 */
export function findSuspiciousTestAttrValue(
  attr: string,
  value: string,
): SuspiciousReason | null {
  if (value === '') {
    return { attr, value, reason: 'empty value' }
  }

  // Emails / user handles
  if (value.includes('@')) {
    return { attr, value, reason: 'contains @ (possible email / PII)' }
  }

  // Auth material
  if (/^(bearer|basic)\s/i.test(value)) {
    return { attr, value, reason: 'looks like an Authorization header' }
  }
  if (/^eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+/.test(value)) {
    return { attr, value, reason: 'looks like a JWT' }
  }

  // Password-ish / secret-ish literals (value itself, not attribute name)
  if (
    /^(password|passwd|secret|api[_-]?key|private[_-]?key|token)=/i.test(value)
  ) {
    return { attr, value, reason: 'looks like an embedded secret assignment' }
  }

  if (attr === 'data-testid') {
    if (!TEST_ID_RE.test(value)) {
      return {
        attr,
        value,
        reason: 'data-testid must be static kebab-case (no PII interpolated)',
      }
    }
    // Reject ids that interpolate UUIDs / high-entropy suffixes (e.g. folder-count-<uuid>)
    if (
      /(?:^|-)[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
        value,
      )
    ) {
      return {
        attr,
        value,
        reason:
          'data-testid must not interpolate entity UUIDs — use a static id plus data-entity-id',
      }
    }
    if (value.length > 80) {
      return {
        attr,
        value,
        reason: 'data-testid unreasonably long (possible interpolated secret)',
      }
    }
    return null
  }

  if (attr === 'data-index' || attr === 'data-depth') {
    if (!/^\d{1,6}$/.test(value)) {
      return { attr, value, reason: 'must be a small non-negative integer' }
    }
    return null
  }

  if (attr === 'data-entity-id') {
    // Prefer UUID; also allow other opaque ids already used in API/URLs (no whitespace, no @)
    if (UUID_RE.test(value)) return null
    if (/\s/.test(value)) {
      return { attr, value, reason: 'entity id must not contain whitespace' }
    }
    if (value.length > 128) {
      return { attr, value, reason: 'entity id unreasonably long (possible secret)' }
    }
    // High-entropy blobs that aren't UUID-shaped
    if (
      value.length >= 40 &&
      /^[A-Za-z0-9+/=_-]+$/.test(value) &&
      !SAFE_KEY_RE.test(value)
    ) {
      return {
        attr,
        value,
        reason: 'looks like a high-entropy secret, not an entity id',
      }
    }
    return null
  }

  if (attr === 'data-entity-key') {
    if (!SAFE_KEY_RE.test(value)) {
      return {
        attr,
        value,
        reason:
          'entity key must be a short API/URL key (no spaces, emails, or secrets)',
      }
    }
    // Reject long opaque tokens masquerading as keys
    if (
      value.length >= 40 &&
      /^[A-Za-z0-9+/=]+$/.test(value) &&
      !/[_.:/-]/.test(value)
    ) {
      return {
        attr,
        value,
        reason: 'entity key looks like a high-entropy token',
      }
    }
    return null
  }

  // Other data-* used for tests
  if (attr.startsWith('data-')) {
    if (value.length >= 40 && /^[A-Za-z0-9+/=_-]+$/.test(value) && !UUID_RE.test(value)) {
      return {
        attr,
        value,
        reason: 'possible secret in data-* attribute',
      }
    }
  }

  return null
}

/** Throw if any attr/value pair is suspicious (used by helpers in non-stripped builds). */
export function assertSafeTestAttrs(
  attrs: Record<string, string | undefined>,
): void {
  for (const [attr, value] of Object.entries(attrs)) {
    if (value == null) continue
    const hit = findSuspiciousTestAttrValue(attr, value)
    if (hit) {
      throw new Error(
        `Unsafe test attribute ${hit.attr}="${hit.value}": ${hit.reason}. ` +
          'Never put PII or secrets in data-testid / data-entity-* (see docs/testing/test-ids.md).',
      )
    }
  }
}
