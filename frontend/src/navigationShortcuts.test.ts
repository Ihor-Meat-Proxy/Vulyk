/** @vitest-environment jsdom */
import { describe, expect, it } from 'vitest'
import {
  shouldHandleBackspaceToParent,
  shouldIgnoreNavShortcutForTarget,
} from './navigationShortcuts'

describe('Backspace-to-parent guard', () => {
  it('ignores Backspace inside inputs and textareas', () => {
    const input = document.createElement('input')
    const textarea = document.createElement('textarea')
    expect(shouldIgnoreNavShortcutForTarget(input)).toBe(true)
    expect(shouldIgnoreNavShortcutForTarget(textarea)).toBe(true)
    expect(
      shouldHandleBackspaceToParent({
        key: 'Backspace',
        metaKey: false,
        ctrlKey: false,
        altKey: false,
        repeat: false,
        target: input,
      }),
    ).toBe(false)
  })

  it('allows Backspace on non-editable targets', () => {
    const div = document.createElement('div')
    expect(
      shouldHandleBackspaceToParent({
        key: 'Backspace',
        metaKey: false,
        ctrlKey: false,
        altKey: false,
        repeat: false,
        target: div,
      }),
    ).toBe(true)
  })
})
