/**
 * Pure helpers for the Vulyk Playwright reporter (unit-tested separately).
 */

export type PendingResult = {
  case_key: string
  status: string
  message?: string
  duration_ms?: number
  external_id: string
}

/** Default flush batch size (tests finished before a POST). */
export const RESULT_BATCH_SIZE = 5

/** Max wait before flushing a non-empty buffer (ms). */
export const RESULT_FLUSH_MS = 1500

/** How many unmatched / missing keys to list in a warning sample. */
export const KEY_WARNING_SAMPLE_LIMIT = 5

export type DryRunPhase =
  | 'open'
  | 'start'
  | 'results'
  | 'close'
  | 'resolve-project'

export type EnvLike = Record<string, string | undefined>

export function mapTestStatus(
  status: 'passed' | 'failed' | 'timedOut' | 'interrupted' | 'skipped' | string,
): string {
  switch (status) {
    case 'passed':
      return 'passed'
    case 'failed':
    case 'timedOut':
    case 'interrupted':
      return 'failed'
    case 'skipped':
      return 'skipped'
    default:
      return 'failed'
  }
}

/** Close payload: Playwright interrupted → cancelled. */
export function closeCancelled(suiteStatus: string): boolean {
  return suiteStatus === 'interrupted'
}

/**
 * Dedupe by case_key (last result wins) for serial retries / shared keys.
 * Preserves insertion order of first appearance of each final key.
 */
export function dedupePendingResults(
  rows: PendingResult[],
): PendingResult[] {
  const byKey = new Map<string, PendingResult>()
  for (const row of rows) byKey.set(row.case_key, row)
  return [...byKey.values()]
}

/** Take up to `limit` rows from the front of the buffer (mutates `buffer`). */
export function takeFlushBatch(
  buffer: PendingResult[],
  limit: number = RESULT_BATCH_SIZE,
): PendingResult[] {
  if (buffer.length === 0) return []
  const n = Math.min(limit, buffer.length)
  return buffer.splice(0, n)
}

export function shouldFlushNow(
  buffered: number,
  batchSize: number = RESULT_BATCH_SIZE,
): boolean {
  return buffered >= batchSize
}

/** `VULYK_REPORTER_DRY_RUN=1` (or true/yes) → log lifecycle without POSTing. */
export function isReporterDryRun(env: EnvLike = process.env): boolean {
  const v = (env.VULYK_REPORTER_DRY_RUN ?? '').trim().toLowerCase()
  return v === '1' || v === 'true' || v === 'yes'
}

export function formatDryRunLog(phase: DryRunPhase, detail: string): string {
  return `[vulyk-reporter] dry-run ${phase}: ${detail}`
}

/**
 * Build a warning for API unmatched_case_keys. Warns once per key via
 * `alreadyWarned` (caller adds `newlyWarned` after logging).
 */
export function formatUnmatchedKeysWarning(
  unmatchedKeys: string[],
  alreadyWarned: Set<string>,
  sampleLimit: number = KEY_WARNING_SAMPLE_LIMIT,
): { message: string | null; newlyWarned: string[] } {
  const fresh: string[] = []
  const seen = new Set<string>()
  for (const raw of unmatchedKeys) {
    const key = raw.trim().toUpperCase()
    if (!key || seen.has(key) || alreadyWarned.has(key)) continue
    seen.add(key)
    fresh.push(key)
  }
  if (fresh.length === 0) return { message: null, newlyWarned: [] }

  const sample = fresh.slice(0, sampleLimit)
  const more = fresh.length - sample.length
  const morePart = more > 0 ? ` (+${more} more)` : ''
  const message =
    `[vulyk-reporter] unmatched case keys: ${fresh.length} ` +
    `(sample: ${sample.join(', ')}${morePart}). ` +
    `Ensure the case exists in the project, or annotate the test with ` +
    `{ type: 'vulyk', description: 'KEY' } / tag @KEY.`
  return { message, newlyWarned: fresh }
}

/** One-shot warning for tests missing a vulyk annotation / tag / title key. */
export function formatMissingAnnotationWarning(
  sampleTitles: string[],
  totalMissing: number,
  sampleLimit: number = KEY_WARNING_SAMPLE_LIMIT,
): string {
  const sample = sampleTitles.slice(0, sampleLimit)
  const more = Math.max(0, totalMissing - sample.length)
  const samplePart = sample.map((t) => JSON.stringify(t)).join(', ')
  const morePart = more > 0 ? ` (+${more} more)` : ''
  return (
    `[vulyk-reporter] ${totalMissing} test(s) missing vulyk case key ` +
    `(sample: ${samplePart}${morePart}); results for those tests are skipped. ` +
    `Annotate with { type: 'vulyk', description: 'VQA-123' } or tag @VQA-123 ` +
    `(see docs/testing/playwright-vulyk.md).`
  )
}

/** Synthetic responses when dry-run skips HTTP. */
export function dryRunApiResponse(
  method: string,
  path: string,
  body?: unknown,
): unknown {
  if (method === 'GET' && path === '/api/v1/projects') {
    return []
  }
  if (method === 'POST' && /\/ci\/runs\/[^/]+\/results$/.test(path)) {
    const results = (body as { results?: unknown[] } | undefined)?.results
    const n = Array.isArray(results) ? results.length : 0
    return { inserted: n, unmatched_case_keys: [] as string[] }
  }
  if (method === 'POST' && /\/ci\/runs$/.test(path)) {
    return {
      run: {
        id: 'dry-run-00000000-0000-4000-8000-000000000001',
        status: 'in_progress',
      },
    }
  }
  if (method === 'POST' && /\/ci\/runs\/[^/]+\/(start|close)$/.test(path)) {
    return { status: path.endsWith('/close') ? 'closed' : 'in_progress' }
  }
  return {}
}
