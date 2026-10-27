import { NavLink } from 'react-router-dom'
import { useI18n } from '../i18n'
import { TestIds, testIdProps } from '../testing'

type Props = {
  showReports?: boolean
}

type NavItem = {
  to: string
  end: boolean
  label: string
  testId: string
}

export function BottomNav({ showReports = true }: Props) {
  const { t } = useI18n()
  const items: NavItem[] = [
    {
      to: '/',
      end: true,
      label: t('nav.cases'),
      testId: TestIds.bottomNavCases,
    },
    {
      to: '/runs',
      end: false,
      label: t('nav.runs'),
      testId: TestIds.bottomNavRuns,
    },
    {
      to: '/executions',
      end: false,
      label: t('nav.executions'),
      testId: TestIds.bottomNavExecutions,
    },
  ]
  if (showReports) {
    items.push({
      to: '/reports',
      end: false,
      label: t('nav.reports'),
      testId: TestIds.bottomNavReports,
    })
  }

  return (
    <nav
      className="bottom-nav"
      aria-label={t('a11y.mobilePrimary')}
      {...testIdProps(TestIds.bottomNav)}
    >
      {items.map((item) => (
        <NavLink
          key={item.to}
          to={item.to}
          end={item.end}
          {...testIdProps(item.testId)}
        >
          {item.label}
        </NavLink>
      ))}
    </nav>
  )
}
