import { type RefObject, useEffect, useRef } from 'react'

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

function listFocusable(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (el) => {
      if (el.closest('[aria-hidden="true"]')) return false
      if (el.hasAttribute('disabled') || el.getAttribute('aria-disabled') === 'true') {
        return false
      }
      // Prefer computed style over offsetParent (unreliable in jsdom / for
      // position:fixed descendants).
      const style = window.getComputedStyle(el)
      if (style.display === 'none' || style.visibility === 'hidden') return false
      return true
    },
  )
}

type Options = {
  /** Called when Escape is pressed while the trap is active. */
  onEscape?: () => void
  /** Prefer this element for initial focus; otherwise first focusable / container. */
  initialFocusRef?: RefObject<HTMLElement | null>
}

/**
 * When `active`, moves focus into `containerRef`, cycles Tab within it, handles
 * Escape via `onEscape`, and restores focus to the previously focused element
 * when deactivated or unmounted.
 */
export function useFocusTrap(
  containerRef: RefObject<HTMLElement | null>,
  active: boolean,
  options: Options = {},
): void {
  const { onEscape, initialFocusRef } = options
  const onEscapeRef = useRef(onEscape)
  const initialFocusRefOpt = useRef(initialFocusRef)
  onEscapeRef.current = onEscape
  initialFocusRefOpt.current = initialFocusRef

  useEffect(() => {
    if (!active) return
    const container = containerRef.current
    if (!container) return

    const previouslyFocused =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null

    const focusInitial = () => {
      const preferred = initialFocusRefOpt.current?.current
      if (preferred && container.contains(preferred)) {
        preferred.focus()
        return
      }
      const focusables = listFocusable(container)
      if (focusables[0]) {
        focusables[0].focus()
        return
      }
      if (!container.hasAttribute('tabindex')) {
        container.tabIndex = -1
      }
      container.focus()
    }

    // Defer so the dialog is in the DOM (conditional render / portal).
    const frame = window.requestAnimationFrame(focusInitial)

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        const escape = onEscapeRef.current
        if (!escape) return
        e.preventDefault()
        e.stopPropagation()
        escape()
        return
      }
      if (e.key !== 'Tab') return

      const focusables = listFocusable(container)
      if (focusables.length === 0) {
        e.preventDefault()
        container.focus()
        return
      }

      const first = focusables[0]
      const last = focusables[focusables.length - 1]
      const activeEl = document.activeElement

      if (e.shiftKey) {
        if (activeEl === first || !container.contains(activeEl)) {
          e.preventDefault()
          last.focus()
        }
      } else if (activeEl === last || !container.contains(activeEl)) {
        e.preventDefault()
        first.focus()
      }
    }

    document.addEventListener('keydown', onKeyDown, true)
    return () => {
      window.cancelAnimationFrame(frame)
      document.removeEventListener('keydown', onKeyDown, true)
      if (
        previouslyFocused &&
        document.contains(previouslyFocused) &&
        typeof previouslyFocused.focus === 'function'
      ) {
        previouslyFocused.focus()
      }
    }
  }, [active, containerRef])
}
