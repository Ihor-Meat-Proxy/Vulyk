/** Cancelable flag + AbortSignal for in-flight async loads after navigation. */
export type StaleGuard = {
  isCancelled: () => boolean
  cancel: () => void
  /** Abort in-flight `fetch` calls that accept this signal. */
  signal: AbortSignal
}

export function createStaleGuard(): StaleGuard {
  const ac = new AbortController()
  let cancelled = false
  return {
    isCancelled: () => cancelled,
    cancel: () => {
      cancelled = true
      ac.abort()
    },
    signal: ac.signal,
  }
}

/** Run `apply` only when this load is still the active one. */
export function applyUnlessCancelled(
  guard: StaleGuard | undefined,
  apply: () => void,
): void {
  if (guard?.isCancelled()) return
  apply()
}
