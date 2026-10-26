import { useEffect, type RefObject } from 'react'
import { shouldHandleSaveShortcut } from '../caseEditShortcuts'
import { isApplePlatform } from '../platform'

/**
 * Registers Ctrl/Cmd+S only while the case editor is mounted (`enabled`).
 * Listener is removed on unmount / when editing ends so the browser Save Page
 * works normally elsewhere. Ignores key repeat, save-in-flight, foreign modals,
 * and non-primary modifier combos (Shift+Cmd+S, etc.).
 */
export function useCaseEditorSaveShortcut(opts: {
  enabled: boolean
  saving: boolean
  editorRef: RefObject<Element | null>
  onSave: () => void
}) {
  const { enabled, saving, editorRef, onSave } = opts

  useEffect(() => {
    if (!enabled) return

    const apple = isApplePlatform()

    const onKeyDown = (e: KeyboardEvent) => {
      if (
        !shouldHandleSaveShortcut(
          e,
          {
            editorRoot: editorRef.current,
            saving,
            document,
          },
          apple,
        )
      ) {
        return
      }
      e.preventDefault()
      onSave()
    }

    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [enabled, saving, editorRef, onSave])
}
