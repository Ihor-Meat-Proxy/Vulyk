import { NavLink } from 'react-router-dom'
import { useI18n } from '../i18n'

type Props = {
  showAdmin?: boolean
  showReports?: boolean
}

type NavItem = { to: string; end: boolean; label: string }

export function BottomNav({ showAdmin = true, showReports = true }: Props) {
  const { t } = useI18n()
  const items: NavItem[] = [
    { to: '/', end: true, label: t('nav.cases') },
    { to: '/runs', end: false, label: t('nav.runs') },
  ]
  if (showReports) {
    items.push({ to: '/reports', end: false, label: t('nav.reports') })
  }
  if (showAdmin) {
    items.push({ to: '/admin', end: false, label: t('nav.admin') })
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
