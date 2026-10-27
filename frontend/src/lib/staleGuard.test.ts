import { describe, expect, it, vi } from 'vitest'

import { applyUnlessCancelled, createStaleGuard } from './staleGuard'

describe('staleGuard', () => {
  it('allows apply until cancelled', () => {
    const guard = createStaleGuard()
    const apply = vi.fn()
    applyUnlessCancelled(guard, apply)
    expect(apply).toHaveBeenCalledTimes(1)
    guard.cancel()
    applyUnlessCancelled(guard, apply)
    expect(apply).toHaveBeenCalledTimes(1)
  })

  it('treats missing guard as current', () => {
    const apply = vi.fn()
    applyUnlessCancelled(undefined, apply)
    expect(apply).toHaveBeenCalledTimes(1)
  })

  it('exposes an AbortSignal aborted by cancel()', () => {
    const guard = createStaleGuard()
    expect(guard.signal.aborted).toBe(false)
    guard.cancel()
    expect(guard.signal.aborted).toBe(true)
  })
})
