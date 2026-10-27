/** @vitest-environment jsdom */
import { createRef, type RefObject } from 'react'
import { act, useRef } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useFocusTrap } from './useFocusTrap'

function TrapHost({
  active,
  onEscape,
}: {
  active: boolean
  onEscape?: () => void
}) {
  const ref = useRef<HTMLDivElement>(null)
  useFocusTrap(ref, active, { onEscape })
  return (
    <div>
      <button type="button" id="outside">
        Outside
      </button>
      <div ref={ref} role="dialog" aria-modal="true" id="dialog">
        <button type="button" id="first">
          First
        </button>
        <button type="button" id="second">
          Second
        </button>
        <button type="button" id="last">
          Last
        </button>
      </div>
    </div>
  )
}

describe('useFocusTrap', () => {
  let container: HTMLDivElement
  let root: Root

  beforeEach(() => {
    container = document.createElement('div')
    document.body.appendChild(container)
    root = createRoot(container)
  })

  afterEach(() => {
    act(() => {
      root.unmount()
    })
    container.remove()
  })

  it('moves focus into the dialog when activated and restores on close', async () => {
    const outside = document.createElement('button')
    outside.id = 'pre'
    document.body.appendChild(outside)
    outside.focus()
    expect(document.activeElement).toBe(outside)

    await act(async () => {
      root.render(<TrapHost active />)
    })
    await act(async () => {
      await new Promise((r) => requestAnimationFrame(r))
    })

    expect(document.activeElement?.id).toBe('first')

    await act(async () => {
      root.render(<TrapHost active={false} />)
    })
    expect(document.activeElement).toBe(outside)
    outside.remove()
  })

  it('cycles Tab and Shift+Tab within the dialog', async () => {
    await act(async () => {
      root.render(<TrapHost active />)
    })
    await act(async () => {
      await new Promise((r) => requestAnimationFrame(r))
    })

    const first = container.querySelector('#first') as HTMLButtonElement
    const last = container.querySelector('#last') as HTMLButtonElement
    expect(document.activeElement).toBe(first)

    await act(async () => {
      document.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Tab', bubbles: true }),
      )
    })
    // Default browser tab order isn't simulated in jsdom; force last then wrap.
    last.focus()
    await act(async () => {
      document.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'Tab',
          bubbles: true,
          cancelable: true,
        }),
      )
    })
    expect(document.activeElement).toBe(first)

    first.focus()
    await act(async () => {
      document.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'Tab',
          shiftKey: true,
          bubbles: true,
          cancelable: true,
        }),
      )
    })
    expect(document.activeElement).toBe(last)
  })

  it('calls onEscape for Escape', async () => {
    const onEscape = vi.fn()
    await act(async () => {
      root.render(<TrapHost active onEscape={onEscape} />)
    })
    await act(async () => {
      document.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'Escape',
          bubbles: true,
          cancelable: true,
        }),
      )
    })
    expect(onEscape).toHaveBeenCalledTimes(1)
  })

  it('no-ops when inactive', async () => {
    const ref: RefObject<HTMLDivElement | null> = createRef()
    function Host() {
      useFocusTrap(ref, false)
      return (
        <div ref={ref}>
          <button type="button" id="inside">
            Inside
          </button>
        </div>
      )
    }
    const outside = document.createElement('button')
    document.body.appendChild(outside)
    outside.focus()
    await act(async () => {
      root.render(<Host />)
    })
    await act(async () => {
      await new Promise((r) => requestAnimationFrame(r))
    })
    expect(document.activeElement).toBe(outside)
    outside.remove()
  })
})
