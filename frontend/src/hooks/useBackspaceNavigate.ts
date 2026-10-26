import { useEffect } from 'react'

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  const tag = target.tagName
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true
  if (target.isContentEditable) return true
  if (target.closest('[contenteditable="true"]')) return true
  if (target.closest('[role="dialog"]')) return true
  if (target.closest('dialog')) return true
  return false
}

/**
 * Navigate to parent when Backspace is pressed outside editable/dialog contexts.
 */
export function useBackspaceNavigate(onBack: () => void, enabled = true) {
  useEffect(() => {
    if (!enabled) return
    const handler = (e: KeyboardEvent) => {
      if (e.key !== 'Backspace') return
      if (e.metaKey || e.ctrlKey || e.altKey) return
      if (isTypingTarget(e.target)) return
      e.preventDefault()
      onBack()
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [onBack, enabled])
}
