import { NavLink } from 'react-router-dom'
import { useI18n } from '../i18n'

type Props = {
  showReports?: boolean
}

type NavItem = { to: string; end: boolean; label: string }

export function BottomNav({ showReports = true }: Props) {
  const { t } = useI18n()
  const items: NavItem[] = [
    { to: '/', end: true, label: t('nav.cases') },
    { to: '/runs', end: false, label: t('nav.runs') },
    { to: '/executions', end: false, label: t('nav.executions') },
  ]
  if (showReports) {
    items.push({ to: '/reports', end: false, label: t('nav.reports') })
  }

  return (
    <nav className="bottom-nav" aria-label="Mobile primary">
      {items.map((item) => (
        <NavLink key={item.to} to={item.to} end={item.end}>
          {item.label}
        </NavLink>
      ))}
    </nav>
  )
}
