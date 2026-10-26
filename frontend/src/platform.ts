/** Detect Apple platforms for ⌘ vs Ctrl shortcut labels and modifiers. */
export function isApplePlatform(
  nav: Pick<Navigator, 'platform' | 'userAgent'> = typeof navigator !== 'undefined'
    ? navigator
    : { platform: '', userAgent: '' },
): boolean {
  const platform = nav.platform || ''
  const ua = nav.userAgent || ''
  return /Mac|iPhone|iPad|iPod/i.test(platform) || /Mac OS X|iPhone|iPad|iPod/i.test(ua)
}

/** Platform-appropriate modifier key label for save/add-step hints. */
export function modKeyLabel(apple = isApplePlatform()): string {
  return apple ? '⌘' : 'Ctrl'
}

/**
 * True when the event target is (or is inside) an editable field or dialog.
 * Used so Backspace-to-parent and similar nav shortcuts never fire while typing.
 */
export function isEditableTarget(target: EventTarget | null): boolean {
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
 * True when a modal/dialog is open that is not scoped to `editorRoot`.
 * Used so Ctrl/Cmd+S does not steal the browser Save Page while other dialogs are up.
 */
export function isForeignModalOpen(
  editorRoot: Element | null,
  doc: Document = typeof document !== 'undefined' ? document : (null as unknown as Document),
): boolean {
  if (!doc?.querySelectorAll) return false
  const modals = doc.querySelectorAll('[role="dialog"], [aria-modal="true"]')
  for (const modal of modals) {
    if (editorRoot && (modal === editorRoot || editorRoot.contains(modal))) continue
    return true
  }
  return false
}
