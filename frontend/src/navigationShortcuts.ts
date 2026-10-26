import { isEditableTarget } from './platform'

/**
 * Guard for Backspace-to-parent (and similar) navigation shortcuts.
 * Never treat Backspace as navigation while focus is in an editable field.
 */
export function shouldIgnoreNavShortcutForTarget(
  target: EventTarget | null,
): boolean {
  return isEditableTarget(target)
}

/** True when Backspace may navigate to the parent route (not inside inputs). */
export function shouldHandleBackspaceToParent(
  e: Pick<KeyboardEvent, 'key' | 'metaKey' | 'ctrlKey' | 'altKey' | 'repeat'> & {
    target: EventTarget | null
  },
): boolean {
  if (e.repeat) return false
  if (e.key !== 'Backspace') return false
  if (e.metaKey || e.ctrlKey || e.altKey) return false
  if (shouldIgnoreNavShortcutForTarget(e.target)) return false
  return true
}
