/** Cancelable flag for in-flight async loads that must not paint after navigation. */
export type StaleGuard = {
  isCancelled: () => boolean
  cancel: () => void
}

export function createStaleGuard(): StaleGuard {
  let cancelled = false
  return {
    isCancelled: () => cancelled,
    cancel: () => {
      cancelled = true
    },
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
