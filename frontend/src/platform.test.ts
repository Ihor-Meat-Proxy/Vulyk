/** @vitest-environment jsdom */
import { describe, expect, it } from 'vitest'
import {
  isApplePlatform,
  isEditableTarget,
  isForeignModalOpen,
  modKeyLabel,
} from './platform'

describe('isApplePlatform', () => {
  it('detects Mac platform', () => {
    expect(isApplePlatform({ platform: 'MacIntel', userAgent: '' })).toBe(true)
  })

  it('detects iPhone via userAgent', () => {
    expect(
      isApplePlatform({
        platform: '',
        userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)',
      }),
    ).toBe(true)
  })

  it('returns false for Windows/Linux', () => {
    expect(
      isApplePlatform({
        platform: 'Win32',
        userAgent: 'Mozilla/5.0 (Windows NT 10.0)',
      }),
    ).toBe(false)
    expect(
      isApplePlatform({
        platform: 'Linux x86_64',
        userAgent: 'Mozilla/5.0 (X11; Linux x86_64)',
      }),
    ).toBe(false)
  })
})

describe('modKeyLabel', () => {
  it('returns ⌘ on Apple and Ctrl elsewhere', () => {
    expect(modKeyLabel(true)).toBe('⌘')
    expect(modKeyLabel(false)).toBe('Ctrl')
  })
})

describe('isEditableTarget', () => {
  it('detects input/textarea/select', () => {
    const input = document.createElement('input')
    const textarea = document.createElement('textarea')
    const select = document.createElement('select')
    const div = document.createElement('div')
    expect(isEditableTarget(input)).toBe(true)
    expect(isEditableTarget(textarea)).toBe(true)
    expect(isEditableTarget(select)).toBe(true)
    expect(isEditableTarget(div)).toBe(false)
    expect(isEditableTarget(null)).toBe(false)
  })
})

describe('isForeignModalOpen', () => {
  it('ignores modals inside the editor root', () => {
    const root = document.createElement('div')
    const dialog = document.createElement('div')
    dialog.setAttribute('role', 'dialog')
    root.appendChild(dialog)
    document.body.appendChild(root)
    expect(isForeignModalOpen(root, document)).toBe(false)
    root.remove()
  })

  it('detects dialogs outside the editor', () => {
    const root = document.createElement('div')
    const foreign = document.createElement('div')
    foreign.setAttribute('aria-modal', 'true')
    document.body.appendChild(root)
    document.body.appendChild(foreign)
    expect(isForeignModalOpen(root, document)).toBe(true)
    root.remove()
    foreign.remove()
  })
})
