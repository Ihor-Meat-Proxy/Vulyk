/** Statuses that mean a run is still receiving results (poll the UI). */
const LIVE_STATUSES = new Set(['in_progress', 'running', 'in-progress'])

/** Default poll cadence — mirrors Admin Updates active-job polling. */
export const DEFAULT_LIVE_RUN_POLL_MS = 2000

/**
 * Resolve poll interval. `VITE_LIVE_RUN_POLL_MS` shortens waits in e2e/dev;
 * invalid or missing values fall back to {@link DEFAULT_LIVE_RUN_POLL_MS}.
 */
export function resolveLiveRunPollMs(
  env: Record<string, string | undefined> = import.meta.env as Record<
    string,
    string | undefined
  >,
): number {
  const raw = env.VITE_LIVE_RUN_POLL_MS
  const n = raw != null && raw !== '' ? Number(raw) : Number.NaN
  return Number.isFinite(n) && n > 0 ? n : DEFAULT_LIVE_RUN_POLL_MS
}

export const LIVE_RUN_POLL_MS = resolveLiveRunPollMs()

/** True when run lifecycle is still live (`in_progress` / `running`). */
export function isLiveRunStatus(status: string | null | undefined): boolean {
  if (!status) return false
  return LIVE_STATUSES.has(status.trim().toLowerCase())
}

/** True when any listed run should keep the list polling. */
export function hasLiveRuns(runs: ReadonlyArray<{ status: string }>): boolean {
  return runs.some((r) => isLiveRunStatus(r.status))
}
