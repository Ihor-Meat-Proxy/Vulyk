import {
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react'
import { LOCALES, useI18n, type Locale } from '../i18n'
import { TestIds, listItemProps, testIdProps } from '../testing'

function GlobeIcon() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.75"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <circle cx="12" cy="12" r="9" />
      <path d="M3 12h18" />
      <path d="M12 3a14 14 0 0 1 0 18" />
      <path d="M12 3a14 14 0 0 0 0 18" />
    </svg>
  )
}

export function LanguageMenu() {
  const { t, locale, setLocale } = useI18n()
  const [open, setOpen] = useState(false)
  const rootRef = useRef<HTMLDivElement>(null)
  const listRef = useRef<HTMLUListElement>(null)
  const menuId = useId()
  const label = t('nav.locale')

  useEffect(() => {
    if (!open) return
    const onDoc = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false)
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setOpen(false)
        rootRef.current?.querySelector<HTMLButtonElement>('button')?.focus()
      }
    }
    document.addEventListener('mousedown', onDoc)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('mousedown', onDoc)
      document.removeEventListener('keydown', onKey)
    }
  }, [open])

  useEffect(() => {
    if (!open) return
    const active = listRef.current?.querySelector<HTMLElement>(
      '[aria-checked="true"]',
    )
    active?.focus()
  }, [open])

  const choose = (code: Locale) => {
    void setLocale(code)
    setOpen(false)
  }

  const onMenuKeyDown = (e: ReactKeyboardEvent) => {
    const items = Array.from(
      listRef.current?.querySelectorAll<HTMLElement>('[role="menuitemradio"]') ??
        [],
    )
    const idx = items.indexOf(document.activeElement as HTMLElement)
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      items[(idx + 1) % items.length]?.focus()
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      items[(idx - 1 + items.length) % items.length]?.focus()
    } else if (e.key === 'Home') {
      e.preventDefault()
      items[0]?.focus()
    } else if (e.key === 'End') {
      e.preventDefault()
      items[items.length - 1]?.focus()
    }
  }

  return (
    <div className="icon-menu" ref={rootRef}>
      <button
        type="button"
        className="icon-btn"
        aria-label={label}
        title={label}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={menuId}
        onClick={() => setOpen((v) => !v)}
        {...testIdProps(TestIds.headerLanguageMenu)}
      >
        <GlobeIcon />
      </button>
      {open ? (
        <ul
          id={menuId}
          className="icon-menu-panel"
          role="menu"
          aria-label={label}
          ref={listRef}
          onKeyDown={onMenuKeyDown}
          {...testIdProps(TestIds.headerLanguageMenuPanel)}
        >
          {LOCALES.map((meta, index) => {
            const checked = meta.code === locale
            return (
              <li key={meta.code} role="none">
                <button
                  type="button"
                  role="menuitemradio"
                  aria-checked={checked}
                  className={checked ? 'is-active' : undefined}
                  tabIndex={checked ? 0 : -1}
                  onClick={() => choose(meta.code)}
                  {...listItemProps(TestIds.headerLanguageItem, {
                    entityId: meta.code,
                    entityKey: meta.code,
                    index,
                  })}
                >
                  <span className="icon-menu-check" aria-hidden="true">
                    {checked ? '✓' : ''}
                  </span>
                  <span>{meta.nativeName}</span>
                </button>
              </li>
            )
          })}
        </ul>
      ) : null}
    </div>
  )
}
