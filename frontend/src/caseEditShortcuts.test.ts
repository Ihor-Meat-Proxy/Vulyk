/** @vitest-environment jsdom */
import { describe, expect, it, vi } from 'vitest'
import {
  isCaseEditorSaveShortcut,
  shouldHandleSaveShortcut,
  shouldModEnterAddStep,
  shouldTabAddStep,
  stepHasContent,
} from './caseEditShortcuts'

function key(
  partial: Partial<KeyboardEvent> & { key: string },
): Pick<
  KeyboardEvent,
  'key' | 'metaKey' | 'ctrlKey' | 'shiftKey' | 'altKey' | 'repeat'
> {
  return {
    key: partial.key,
    metaKey: partial.metaKey ?? false,
    ctrlKey: partial.ctrlKey ?? false,
    shiftKey: partial.shiftKey ?? false,
    altKey: partial.altKey ?? false,
    repeat: partial.repeat ?? false,
  }
}

describe('stepHasContent', () => {
  it('is true when action or expected is non-empty', () => {
    expect(stepHasContent({ action: 'a', expected_result: '' })).toBe(true)
    expect(stepHasContent({ action: '', expected_result: 'e' })).toBe(true)
    expect(stepHasContent({ action: '  ', expected_result: '' })).toBe(false)
    expect(stepHasContent(undefined)).toBe(false)
  })
})

describe('shouldTabAddStep', () => {
  const base = {
    stepIndex: 0,
    stepsLength: 1,
    step: { action: 'do', expected_result: '' },
    field: 'expected' as const,
  }

  it('adds on Tab in last Expected when step has content', () => {
    expect(shouldTabAddStep(key({ key: 'Tab' }), base)).toBe(true)
  })

  it('does not add on Shift+Tab', () => {
    expect(shouldTabAddStep(key({ key: 'Tab', shiftKey: true }), base)).toBe(
      false,
    )
  })

  it('does not add on non-last steps', () => {
    expect(
      shouldTabAddStep(key({ key: 'Tab' }), {
        ...base,
        stepIndex: 0,
        stepsLength: 2,
      }),
    ).toBe(false)
  })

  it('does not add when last step is empty', () => {
    expect(
      shouldTabAddStep(key({ key: 'Tab' }), {
        ...base,
        step: { action: '', expected_result: '' },
      }),
    ).toBe(false)
  })

  it('does not add from Action field', () => {
    expect(
      shouldTabAddStep(key({ key: 'Tab' }), { ...base, field: 'action' }),
    ).toBe(false)
  })
})

describe('shouldModEnterAddStep', () => {
  it('matches Cmd+Enter on Apple and Ctrl+Enter elsewhere', () => {
    expect(
      shouldModEnterAddStep(key({ key: 'Enter', metaKey: true }), true),
    ).toBe(true)
    expect(
      shouldModEnterAddStep(key({ key: 'Enter', ctrlKey: true }), false),
    ).toBe(true)
    expect(
      shouldModEnterAddStep(key({ key: 'Enter', ctrlKey: true }), true),
    ).toBe(false)
    expect(
      shouldModEnterAddStep(key({ key: 'Enter', metaKey: true }), false),
    ).toBe(false)
  })

  it('ignores repeat and Shift/Alt', () => {
    expect(
      shouldModEnterAddStep(
        key({ key: 'Enter', metaKey: true, repeat: true }),
        true,
      ),
    ).toBe(false)
    expect(
      shouldModEnterAddStep(
        key({ key: 'Enter', ctrlKey: true, shiftKey: true }),
        false,
      ),
    ).toBe(false)
  })
})

describe('isCaseEditorSaveShortcut', () => {
  it('matches Cmd+S on Apple and Ctrl+S elsewhere', () => {
    expect(isCaseEditorSaveShortcut(key({ key: 's', metaKey: true }), true)).toBe(
      true,
    )
    expect(
      isCaseEditorSaveShortcut(key({ key: 'S', ctrlKey: true }), false),
    ).toBe(true)
  })

  it('rejects Shift+Cmd+S, Alt+S, Ctrl+S on Apple, and key repeat', () => {
    expect(
      isCaseEditorSaveShortcut(
        key({ key: 's', metaKey: true, shiftKey: true }),
        true,
      ),
    ).toBe(false)
    expect(
      isCaseEditorSaveShortcut(
        key({ key: 's', metaKey: true, altKey: true }),
        true,
      ),
    ).toBe(false)
    expect(
      isCaseEditorSaveShortcut(key({ key: 's', ctrlKey: true }), true),
    ).toBe(false)
    expect(
      isCaseEditorSaveShortcut(
        key({ key: 's', metaKey: true, repeat: true }),
        true,
      ),
    ).toBe(false)
  })
})

describe('shouldHandleSaveShortcut', () => {
  it('preventDefault gate: only when editor mounted, not saving, no foreign modal', () => {
    const editorRoot = document.createElement('div')
    document.body.appendChild(editorRoot)

    const saveEvent = key({ key: 's', ctrlKey: true })
    expect(
      shouldHandleSaveShortcut(saveEvent, {
        editorRoot,
        saving: false,
        document,
      }, false),
    ).toBe(true)

    expect(
      shouldHandleSaveShortcut(saveEvent, {
        editorRoot: null,
        saving: false,
        document,
      }, false),
    ).toBe(false)

    expect(
      shouldHandleSaveShortcut(saveEvent, {
        editorRoot,
        saving: true,
        document,
      }, false),
    ).toBe(false)

    const foreign = document.createElement('div')
    foreign.setAttribute('role', 'dialog')
    document.body.appendChild(foreign)
    expect(
      shouldHandleSaveShortcut(saveEvent, {
        editorRoot,
        saving: false,
        document,
      }, false),
    ).toBe(false)

    foreign.remove()
    editorRoot.remove()
  })

  it('asserts preventDefault is applied only when the gate accepts the event', () => {
    const editorRoot = document.createElement('div')
    document.body.appendChild(editorRoot)

    const makeEvent = () => {
      const e = new KeyboardEvent('keydown', {
        key: 's',
        ctrlKey: true,
        bubbles: true,
        cancelable: true,
      })
      const preventDefault = vi.spyOn(e, 'preventDefault')
      return { e, preventDefault }
    }

    const accepted = makeEvent()
    if (
      shouldHandleSaveShortcut(accepted.e, {
        editorRoot,
        saving: false,
        document,
      }, false)
    ) {
      accepted.e.preventDefault()
    }
    expect(accepted.preventDefault).toHaveBeenCalledTimes(1)

    editorRoot.remove()
    const rejected = makeEvent()
    if (
      shouldHandleSaveShortcut(rejected.e, {
        editorRoot: null,
        saving: false,
        document,
      }, false)
    ) {
      rejected.e.preventDefault()
    }
    expect(rejected.preventDefault).not.toHaveBeenCalled()
  })
})
