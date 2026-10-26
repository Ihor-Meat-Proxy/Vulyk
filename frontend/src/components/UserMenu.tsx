import {
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { useAuth } from '../auth'
import { useI18n } from '../i18n'

function GearIcon() {
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
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9c.2.6.7 1 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" />
    </svg>
  )
}

function initials(name: string, email: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean)
  if (parts.length >= 2) {
    return (parts[0]![0]! + parts[1]![0]!).toUpperCase()
  }
  if (parts.length === 1 && parts[0]!.length >= 2) {
    return parts[0]!.slice(0, 2).toUpperCase()
  }
  return (email || '?').slice(0, 2).toUpperCase()
}

type Props = {
  showAdmin: boolean
  docsUrl: string
}

export function UserMenu({ showAdmin, docsUrl }: Props) {
  const { user, status, logout } = useAuth()
  const { t } = useI18n()
  const navigate = useNavigate()
  const [open, setOpen] = useState(false)
  const rootRef = useRef<HTMLDivElement>(null)
  const listRef = useRef<HTMLDivElement>(null)
  const menuId = useId()
  const label = t('nav.userMenu')
  const authRequired = Boolean(status?.auth_required)
  const modeLabel = authRequired ? t('nav.authRequired') : t('nav.openMode')

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
    const first = listRef.current?.querySelector<HTMLElement>(
      '[role="menuitem"]',
    )
    first?.focus()
  }, [open])

  const close = () => setOpen(false)

  const onMenuKeyDown = (e: ReactKeyboardEvent) => {
    const items = Array.from(
      listRef.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [],
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

  const go = (path: string) => {
    close()
    navigate(path)
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
      >
        <GearIcon />
      </button>
      {open ? (
        <div
          id={menuId}
          className="icon-menu-panel user-menu-panel"
          role="menu"
          aria-label={label}
          ref={listRef}
          onKeyDown={onMenuKeyDown}
        >
          <div className="user-menu-header" role="presentation">
            <span className="user-avatar" aria-hidden="true">
              {user
                ? initials(user.display_name, user.email)
                : '?'}
            </span>
            <div className="user-menu-identity">
              <strong>
                {user?.display_name ?? status?.instance_name ?? 'Vulyk'}
              </strong>
              <span className="muted">
                {user?.email ?? t('nav.modeInfo')}
              </span>
            </div>
          </div>

          <div className="user-menu-mode" role="presentation">
            <span className="mode-badge" title={t('nav.modeInfo')}>
              {modeLabel}
            </span>
          </div>

          {user ? (
            <button
              type="button"
              role="menuitem"
              className="user-menu-item"
              onClick={() => go('/settings')}
            >
              {t('nav.mySettings')}
            </button>
          ) : null}

          <button
            type="button"
            role="menuitem"
            className="user-menu-item"
            onClick={() => go('/appearance')}
          >
            {t('nav.appearance')}
          </button>

          {showAdmin ? (
            <button
              type="button"
              role="menuitem"
              className="user-menu-item"
              onClick={() => go('/admin')}
            >
              {t('nav.adminSettings')}
            </button>
          ) : null}

          <a
            role="menuitem"
            className="user-menu-item"
            href={docsUrl}
            target="_blank"
            rel="noreferrer"
            onClick={close}
          >
            {t('nav.helpDocs')}
          </a>

          <div className="user-menu-sep" role="separator" />

          {user ? (
            <button
              type="button"
              role="menuitem"
              className="user-menu-item"
              onClick={() => {
                close()
                void logout()
              }}
            >
              {t('nav.signOut')}
            </button>
          ) : (
            <Link
              role="menuitem"
              className="user-menu-item"
              to="/login"
              onClick={close}
            >
              {t('nav.signIn')}
            </Link>
          )}
        </div>
      ) : null}
    </div>
  )
}
