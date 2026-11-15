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
