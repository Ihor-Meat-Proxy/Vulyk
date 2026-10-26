import { Link } from 'react-router-dom'
import type { Suite } from '../api'
import { useI18n } from '../i18n'
import { casePath } from '../lib/caseUrls'

export type BreadcrumbSeg = {
  id: string
  label: string
  to: string
  current?: boolean
}

type Props = {
  projectName: string
  suites: Suite[]
  suiteId: string | null
  caseKey: string
  caseTitle: string
  onSelectFolder: (suiteId: string | null) => void
}

function buildFolderChain(
  suites: Suite[],
  suiteId: string | null,
): Suite[] {
  if (!suiteId) return []
  const byId = new Map(suites.map((s) => [s.id, s]))
  const chain: Suite[] = []
  let cur: Suite | undefined = byId.get(suiteId)
  const guard = new Set<string>()
  while (cur && !guard.has(cur.id)) {
    guard.add(cur.id)
    chain.unshift(cur)
    cur = cur.parent_id ? byId.get(cur.parent_id) : undefined
  }
  return chain
}

export function CaseBreadcrumbs({
  projectName,
  suites,
  suiteId,
  caseKey,
  caseTitle,
  onSelectFolder,
}: Props) {
  const { t } = useI18n()
  const folders = buildFolderChain(suites, suiteId)

  const segments: BreadcrumbSeg[] = [
    {
      id: 'project',
      label: projectName,
      to: '/',
    },
    ...folders.map((f) => ({
      id: f.id,
      label: f.name,
      to: `/?suite=${encodeURIComponent(f.id)}`,
    })),
    {
      id: 'case',
      label: `${caseKey} ${caseTitle}`,
      to: casePath(caseKey),
      current: true,
    },
  ]

  return (
    <nav
      className="case-breadcrumbs"
      aria-label={t('cases.breadcrumbNav')}
      data-testid="case-detail-breadcrumbs"
    >
      <ol className="case-breadcrumbs-list">
        {segments.map((seg, i) => {
          const isLast = i === segments.length - 1
          return (
            <li key={seg.id} className="case-breadcrumbs-item">
              {i > 0 ? (
                <span className="case-breadcrumbs-sep" aria-hidden>
                  /
                </span>
              ) : null}
              {isLast || seg.current ? (
                <span
                  className="case-breadcrumbs-current"
                  title={seg.label}
                  aria-current="page"
                  data-testid="case-detail-breadcrumb-current"
                >
                  {seg.label}
                </span>
              ) : (
                <Link
                  to={seg.to}
                  className="case-breadcrumbs-link"
                  title={seg.label}
                  data-testid={`case-detail-breadcrumb-${seg.id === 'project' ? 'project' : 'folder'}`}
                  onClick={(e) => {
                    if (seg.id === 'project') {
                      e.preventDefault()
                      onSelectFolder(null)
                    } else if (seg.id !== 'case') {
                      e.preventDefault()
                      onSelectFolder(seg.id)
                    }
                  }}
                >
                  {seg.label}
                </Link>
              )}
            </li>
          )
        })}
      </ol>
    </nav>
  )
}
