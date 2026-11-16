/**
 * Copy text to the clipboard without triggering a browser permission prompt
 * when avoidable. Prefers the Clipboard API; falls back to a temporary
 * textarea + `document.execCommand('copy')`.
 */
export async function writeClipboard(text: string): Promise<void> {
  if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text)
      return
    } catch {
      // Fall through to the legacy path (e.g. insecure context / denied).
    }
  }

  const ta = document.createElement('textarea')
  ta.value = text
  ta.setAttribute('readonly', '')
  ta.style.position = 'fixed'
  ta.style.left = '-9999px'
  document.body.appendChild(ta)
  ta.select()
  const ok = document.execCommand('copy')
  document.body.removeChild(ta)
  if (!ok) {
    throw new Error('clipboard write failed')
  }
}
