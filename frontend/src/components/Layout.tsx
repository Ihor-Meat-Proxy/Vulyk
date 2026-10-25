import { useEffect, useId, useState } from 'react'
import { NavLink, Outlet, useLocation } from 'react-router-dom'
import type { Project, Suite } from '../api'
import { useAuth } from '../auth'
import { useProjectPermissions } from '../hooks/useProjectPermissions'
import { useI18n, type Locale } from '../i18n'
import { BottomNav } from './BottomNav'
import { PwaToasts } from './PwaToasts'

type Props = {
  projects: Project[]
  project: Project | null
  suites: Suite[]
  selectedSuiteId: string | null
  onSelectProject: (id: string) => void
  onSelectSuite: (id: string | null) => void
}

export function Layout({
  projects,
  project,
  suites,
  selectedSuiteId,
  onSelectProject,
  onSelectSuite,
}: Props) {
  const { user, logout, status } = useAuth()
  const { t, locale, setLocale } = useI18n()
  const { can } = useProjectPermissions(project?.id ?? null)
  const location = useLocation()
  const [drawerOpen, setDrawerOpen] = useState(false)
  const drawerTitleId = useId()

  useEffect(() => {
    setDrawerOpen(false)
  }, [location.pathname, selectedSuiteId])

  useEffect(() => {
    if (!drawerOpen) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setDrawerOpen(false)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [drawerOpen])

  const selectSuite = (id: string | null) => {
    onSelectSuite(id)
    setDrawerOpen(false)
  }

  const showImport = can('import_cases')
  const showGenerate = can('llm_assist')
  const showChat = can('llm_assist')
  const showCi = can('trigger_ci') || can('manage_ci_config')
  const showAdmin =
    Boolean(user?.is_system_admin) ||
    can('manage_memberships') ||
    can('manage_api_tokens') ||
    can('manage_llm_config')

  return (
    <div className={`app-shell${drawerOpen ? ' drawer-open' : ''}`}>
      <PwaToasts />

      {drawerOpen && (
        <button
          type="button"
          className="drawer-backdrop"
          aria-label="Close suite drawer"
          onClick={() => setDrawerOpen(false)}
        />
      )}

      <aside
        className="sidebar"
        id="suite-drawer"
        aria-labelledby={drawerTitleId}
      >
        <div className="brand-row">
          <div className="brand">
            <span className="brand-mark" id={drawerTitleId}>
              {status?.instance_name || 'Vulyk'}
            </span>
            <span className="brand-tag">Test management</span>
          </div>
          <button
            type="button"
            className="btn secondary drawer-close"
            onClick={() => setDrawerOpen(false)}
          >
            {t('nav.close')}
          </button>
        </div>

        <nav className="nav nav-desktop" aria-label="Primary">
          <NavLink to="/" end>
            {t('nav.cases')}
          </NavLink>
          {can('manage_requirements') || can('view_project') ? (
            <NavLink to="/requirements">{t('nav.requirements')}</NavLink>
          ) : null}
          {showImport ? <NavLink to="/import">{t('nav.import')}</NavLink> : null}
          {showGenerate ? (
            <NavLink to="/generate">{t('nav.generate')}</NavLink>
          ) : null}
          {showChat ? <NavLink to="/chat">{t('nav.chat')}</NavLink> : null}
          <NavLink to="/runs">{t('nav.runs')}</NavLink>
          {showCi ? <NavLink to="/ci">{t('nav.ci')}</NavLink> : null}
          {can('view_reports') ? (
            <NavLink to="/reports">{t('nav.reports')}</NavLink>
          ) : null}
          {showAdmin ? <NavLink to="/admin">{t('nav.admin')}</NavLink> : null}
        </nav>

        <div className="suite-block">
          <div
            className="muted"
            style={{ marginBottom: '0.5rem', fontSize: '0.8rem' }}
          >
            {t('nav.suites')}
          </div>
          <div className="suite-tree">
            <button
              type="button"
              className={selectedSuiteId === null ? 'active' : undefined}
              onClick={() => selectSuite(null)}
            >
              {t('nav.allCases')}
            </button>
            {suites.map((suite) => (
              <button
                key={suite.id}
                type="button"
                className={selectedSuiteId === suite.id ? 'active' : undefined}
                onClick={() => selectSuite(suite.id)}
              >
                {suite.name}
              </button>
            ))}
            {project && suites.length === 0 && (
              <p
                className="muted"
                style={{ margin: '0.35rem 0.55rem', fontSize: '0.85rem' }}
              >
                No suites yet
              </p>
            )}
          </div>
        </div>
      </aside>

      <div className="main">
        <header className="topbar">
          <div className="topbar-lead">
            <button
              type="button"
              className="btn secondary menu-toggle"
              aria-expanded={drawerOpen}
              aria-controls="suite-drawer"
              onClick={() => setDrawerOpen((v) => !v)}
            >
              {t('nav.suites')}
            </button>
            <h1>{project ? project.name : status?.instance_name || 'Vulyk'}</h1>
          </div>
          <div className="topbar-actions">
            <label>
              <span className="muted" style={{ marginRight: '0.5rem' }}>
                {t('nav.locale')}
              </span>
              <select
                className="project-select"
                value={locale}
                aria-label={t('nav.locale')}
                onChange={(e) => void setLocale(e.target.value as Locale)}
              >
                <option value="en">EN</option>
                <option value="uk">UK</option>
              </select>
            </label>
            <label>
              <span className="muted" style={{ marginRight: '0.5rem' }}>
                {t('nav.project')}
              </span>
              <select
                className="project-select"
                value={project?.id ?? ''}
                onChange={(e) => onSelectProject(e.target.value)}
                disabled={projects.length === 0}
              >
                {projects.length === 0 && <option value="">No projects</option>}
                {projects.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.key} — {p.name}
                  </option>
                ))}
              </select>
            </label>
            {user ? (
              <div className="user-chip">
                <span>{user.display_name}</span>
                <button
                  type="button"
                  className="btn secondary"
                  onClick={() => void logout()}
                >
                  {t('nav.signOut')}
                </button>
              </div>
            ) : status && !status.auth_required ? (
              <span className="muted">{t('nav.openMode')}</span>
            ) : null}
          </div>
        </header>
        <main className="content">
          <Outlet />
        </main>
        <BottomNav showAdmin={showAdmin} showReports={can('view_reports')} />
      </div>
    </div>
  )
}
