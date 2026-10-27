import { useEffect, useId, useState } from 'react'
import { NavLink, Outlet, useLocation } from 'react-router-dom'
import type { Project } from '../api'
import { useAuth } from '../auth'
import { useProjectPermissions } from '../hooks/useProjectPermissions'
import { useI18n } from '../i18n'
import { TestIds, testIdProps } from '../testing'
import { AppVersion } from './AppVersion'
import { BottomNav } from './BottomNav'
import { LanguageMenu } from './LanguageMenu'
import { PwaToasts } from './PwaToasts'
import { SampleBadge } from './SampleBadge'
import { UserMenu } from './UserMenu'

type Props = {
  projects: Project[]
  project: Project | null
  onSelectProject: (id: string) => void
}

function docsUrl(): string {
  const base = (import.meta.env.VITE_API_URL as string | undefined) ?? ''
  return `${base.replace(/\/$/, '')}/api/docs`
}

export function Layout({ projects, project, onSelectProject }: Props) {
  const { user, status } = useAuth()
  const { t } = useI18n()
  const { can } = useProjectPermissions(project?.id ?? null)
  const location = useLocation()
  const [drawerOpen, setDrawerOpen] = useState(false)
  const drawerTitleId = useId()

  useEffect(() => {
    setDrawerOpen(false)
  }, [location.pathname])

  useEffect(() => {
    if (!drawerOpen) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setDrawerOpen(false)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [drawerOpen])

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
    <div
      className={`app-shell${drawerOpen ? ' drawer-open' : ''}`}
      {...testIdProps(TestIds.appShell)}
    >
      <PwaToasts />

      {drawerOpen && (
        <button
          type="button"
          className="drawer-backdrop"
          aria-label={t('nav.closeMenu')}
          onClick={() => setDrawerOpen(false)}
          {...testIdProps(TestIds.navDrawerBackdrop)}
        />
      )}

      <aside
        className="sidebar"
        id="nav-drawer"
        aria-labelledby={drawerTitleId}
        {...testIdProps(TestIds.navDrawer)}
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
            {...testIdProps(TestIds.navDrawerClose)}
          >
            {t('nav.close')}
          </button>
        </div>

        <nav className="nav" aria-label={t('a11y.primaryNav')}>
          <NavLink
            to="/"
            end
            onClick={() => setDrawerOpen(false)}
            {...testIdProps(TestIds.navLinkCases)}
          >
            {t('nav.cases')}
          </NavLink>
          {can('manage_requirements') || can('view_project') ? (
            <NavLink
              to="/requirements"
              onClick={() => setDrawerOpen(false)}
              {...testIdProps(TestIds.navLinkRequirements)}
            >
              {t('nav.requirements')}
            </NavLink>
          ) : null}
          {showImport ? (
            <NavLink
              to="/import"
              onClick={() => setDrawerOpen(false)}
              {...testIdProps(TestIds.navLinkImport)}
            >
              {t('nav.import')}
            </NavLink>
          ) : null}
          {showGenerate ? (
            <NavLink
              to="/generate"
              onClick={() => setDrawerOpen(false)}
              {...testIdProps(TestIds.navLinkGenerate)}
            >
              {t('nav.generate')}
            </NavLink>
          ) : null}
          {showChat ? (
            <NavLink
              to="/chat"
              onClick={() => setDrawerOpen(false)}
              {...testIdProps(TestIds.navLinkChat)}
            >
              {t('nav.chat')}
            </NavLink>
          ) : null}
          <NavLink
            to="/runs"
            onClick={() => setDrawerOpen(false)}
            {...testIdProps(TestIds.navLinkRuns)}
          >
            {t('nav.runs')}
          </NavLink>
          <NavLink
            to="/executions"
            onClick={() => setDrawerOpen(false)}
            {...testIdProps(TestIds.navLinkExecutions)}
          >
            {t('nav.executions')}
          </NavLink>
          {showCi ? (
            <NavLink
              to="/ci"
              onClick={() => setDrawerOpen(false)}
              {...testIdProps(TestIds.navLinkCi)}
            >
              {t('nav.ci')}
            </NavLink>
          ) : null}
          {can('view_reports') ? (
            <NavLink
              to="/reports"
              onClick={() => setDrawerOpen(false)}
              {...testIdProps(TestIds.navLinkReports)}
            >
              {t('nav.reports')}
            </NavLink>
          ) : null}
        </nav>
        <AppVersion />
      </aside>

      <div className="main">
        <header className="topbar">
          <div className="topbar-lead">
            <button
              type="button"
              className="btn secondary menu-toggle"
              aria-expanded={drawerOpen}
              aria-controls="nav-drawer"
              onClick={() => setDrawerOpen((v) => !v)}
              {...testIdProps(TestIds.headerMenuToggle)}
            >
              {t('nav.menu')}
            </button>
            <h1>
              {project ? project.name : status?.instance_name || 'Vulyk'}
              {project?.is_sample ? (
                <>
                  {' '}
                  <SampleBadge testId="header-project-sample-badge" />
                </>
              ) : null}
            </h1>
          </div>
          <div className="topbar-actions">
            <label className="project-field">
              <span className="muted project-field-label">
                {t('nav.project')}
              </span>
              <select
                className="project-select"
                aria-label={t('nav.project')}
                value={project?.id ?? ''}
                onChange={(e) => onSelectProject(e.target.value)}
                disabled={projects.length === 0}
                {...testIdProps(TestIds.headerProjectSelect)}
              >
                {projects.length === 0 && <option value="">No projects</option>}
                {projects.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.key} — {p.name}
                    {p.is_sample ? ' (Sample)' : ''}
                  </option>
                ))}
              </select>
            </label>
            <div className="topbar-icons">
              <LanguageMenu />
              <UserMenu showAdmin={showAdmin} docsUrl={docsUrl()} />
            </div>
          </div>
        </header>
        <main className="content">
          <Outlet />
        </main>
        <BottomNav showReports={can('view_reports')} />
      </div>
    </div>
  )
}
