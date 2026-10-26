import { isApplePlatform, isForeignModalOpen } from './platform'

export type DraftStepLike = {
  action: string
  expected_result: string
}

/** Last step has content if Action or Expected is non-empty (trimmed). */
export function stepHasContent(step: DraftStepLike | undefined): boolean {
  if (!step) return false
  return step.action.trim().length > 0 || step.expected_result.trim().length > 0
}

/**
 * Tab on the Expected field of the last step should add a new empty step
 * when that step already has content. Shift+Tab and non-last steps keep
 * normal focus order; an empty last step does not spawn more.
 */
export function shouldTabAddStep(
  e: Pick<KeyboardEvent, 'key' | 'shiftKey'>,
  opts: {
    stepIndex: number
    stepsLength: number
    step: DraftStepLike
    field: 'action' | 'expected'
  },
): boolean {
  if (e.key !== 'Tab' || e.shiftKey) return false
  if (opts.field !== 'expected') return false
  if (opts.stepIndex !== opts.stepsLength - 1) return false
  return stepHasContent(opts.step)
}

/**
 * Ctrl/Cmd+Enter inside a step field adds the next step.
 * Uses platform-appropriate primary modifier; ignores Shift/Alt/repeat.
 */
export function shouldModEnterAddStep(
  e: Pick<
    KeyboardEvent,
    'key' | 'metaKey' | 'ctrlKey' | 'shiftKey' | 'altKey' | 'repeat'
  >,
  apple = isApplePlatform(),
): boolean {
  if (e.repeat) return false
  if (e.key !== 'Enter') return false
  if (e.shiftKey || e.altKey) return false
  if (apple) {
    return e.metaKey && !e.ctrlKey
  }
  return e.ctrlKey && !e.metaKey
}

/**
 * Strict Ctrl/Cmd+S match for the case editor.
 * - Ignores key repeat
 * - Requires primary platform modifier only (⌘ on Apple, Ctrl elsewhere)
 * - Rejects Shift/Alt and mixed modifiers (e.g. Shift+Cmd+S)
 */
export function isCaseEditorSaveShortcut(
  e: Pick<
    KeyboardEvent,
    'key' | 'metaKey' | 'ctrlKey' | 'shiftKey' | 'altKey' | 'repeat'
  >,
  apple = isApplePlatform(),
): boolean {
  if (e.repeat) return false
  const key = e.key.length === 1 ? e.key.toLowerCase() : e.key
  if (key !== 's') return false
  if (e.shiftKey || e.altKey) return false
  if (apple) {
    return Boolean(e.metaKey) && !e.ctrlKey
  }
  return Boolean(e.ctrlKey) && !e.metaKey
}

export type SaveShortcutGate = {
  /** Editor root element; listener should only be registered while mounted. */
  editorRoot: Element | null
  /** True while a save request is in flight — ignore further shortcuts. */
  saving: boolean
  /** Optional document override for tests. */
  document?: Document
}

/**
 * Whether the save shortcut should be handled (and preventDefault applied).
 * Call only from a listener registered while the case editor is mounted.
 */
export function shouldHandleSaveShortcut(
  e: Pick<
    KeyboardEvent,
    'key' | 'metaKey' | 'ctrlKey' | 'shiftKey' | 'altKey' | 'repeat'
  >,
  gate: SaveShortcutGate,
  apple = isApplePlatform(),
): boolean {
  if (!gate.editorRoot) return false
  if (gate.saving) return false
  if (!isCaseEditorSaveShortcut(e, apple)) return false
  if (isForeignModalOpen(gate.editorRoot, gate.document)) return false
  return true
}
