import { useCallback, useState, type KeyboardEvent } from 'react'
import { useI18n } from '../i18n'
import { casePath } from '../lib/caseUrls'
import { TestIds, testIdProps } from '../testing'
import { useToast } from './ToastProvider'

type Props = {
  caseKey: string
  /** Optional: also offer “copy link” via long-press / secondary control. */
  showCopyLink?: boolean
  className?: string
}

async function writeClipboard(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text)
    return
  }
  const ta = document.createElement('textarea')
  ta.value = text
  ta.setAttribute('readonly', '')
  ta.style.position = 'fixed'
  ta.style.left = '-9999px'
  document.body.appendChild(ta)
  ta.select()
  document.execCommand('copy')
  document.body.removeChild(ta)
}

function CopyIcon() {
  return (
    <svg
      className="case-key-copy-icon"
      width="12"
      height="12"
      viewBox="0 0 16 16"
      aria-hidden
    >
      <rect
        x="5"
        y="5"
        width="8"
        height="8"
        rx="1.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
      />
      <path
        d="M3 11V3.5A1.5 1.5 0 0 1 4.5 2H11"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
    </svg>
  )
}

export function CaseKeyBadge({
  caseKey,
  showCopyLink = true,
  className = '',
}: Props) {
  const { t } = useI18n()
  const { toast } = useToast()
  const [busy, setBusy] = useState(false)

  const copyKey = useCallback(async () => {
    if (busy) return
    setBusy(true)
    try {
      await writeClipboard(caseKey)
      toast(t('cases.copiedKey', { key: caseKey }))
    } catch {
      toast(t('cases.copyFailed'))
    } finally {
      setBusy(false)
    }
  }, [busy, caseKey, t, toast])

  const copyLink = useCallback(async () => {
    if (busy) return
    setBusy(true)
    try {
      const url = `${window.location.origin}${casePath(caseKey)}`
      await writeClipboard(url)
      toast(t('cases.copiedLink', { key: caseKey }))
    } catch {
      toast(t('cases.copyFailed'))
    } finally {
      setBusy(false)
    }
  }, [busy, caseKey, t, toast])

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      void copyKey()
    }
  }

  return (
    <span className={`case-key-badge-wrap ${className}`.trim()}>
      <button
        type="button"
        className="badge case-key-badge"
        title={t('cases.copyKeyTooltip', { key: caseKey })}
        aria-label={t('cases.copyKeyAria', { key: caseKey })}
        onClick={() => void copyKey()}
        onKeyDown={onKeyDown}
        {...testIdProps(TestIds.caseDetailKeyBadge)}
      >
        <span className="case-key-text">{caseKey}</span>
        <CopyIcon />
      </button>
      {showCopyLink ? (
        <button
          type="button"
          className="case-key-copy-link"
          title={t('cases.copyLinkTooltip', { key: caseKey })}
          aria-label={t('cases.copyLinkAria', { key: caseKey })}
          onClick={() => void copyLink()}
          {...testIdProps(TestIds.caseDetailCopyLink)}
        >
          {t('cases.copyLink')}
        </button>
      ) : null}
    </span>
  )
}
