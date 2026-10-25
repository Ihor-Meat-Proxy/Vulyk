import { NavLink } from 'react-router-dom'

const items = [
  { to: '/', end: true, label: 'Cases' },
  { to: '/runs', end: false, label: 'Runs' },
  { to: '/reports', end: false, label: 'Reports' },
  { to: '/admin', end: false, label: 'Admin' },
] as const

export function BottomNav() {
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
