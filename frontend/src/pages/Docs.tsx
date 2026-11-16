import { useEffect, useMemo, useState, type ReactNode } from 'react'
import ReactMarkdown from 'react-markdown'
import { Link, useNavigate, useParams } from 'react-router-dom'
import remarkGfm from 'remark-gfm'
import { useI18n } from '../i18n'
import { TestIds, testIdProps } from '../testing'
import { getUserDocPage, listUserDocPages } from '../userDocs/catalog'
import { extractToc, slugifyHeading } from '../userDocs/toc'

function headingId(children: ReactNode): string {
  const text = Array.isArray(children)
    ? children.map((c) => (typeof c === 'string' ? c : '')).join('')
    : typeof children === 'string'
      ? children
      : ''
  return slugifyHeading(text)
}

export function Docs() {
  const { t } = useI18n()
  const { docId } = useParams<{ docId?: string }>()
  const navigate = useNavigate()
  const pages = useMemo(() => listUserDocPages(), [])
  const [query, setQuery] = useState('')

  const activeId =
    docId && pages.some((p) => p.id === docId) ? docId : pages[0]?.id
  const page = activeId ? getUserDocPage(activeId) : undefined

  useEffect(() => {
    if (!docId && pages[0]) {
      navigate(`/docs/${pages[0].id}`, { replace: true })
    } else if (docId && !pages.some((p) => p.id === docId) && pages[0]) {
      navigate(`/docs/${pages[0].id}`, { replace: true })
    }
  }, [docId, navigate, pages])

  const toc = useMemo(() => (page ? extractToc(page.markdown) : []), [page])

  const filteredPages = useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return pages
    return pages.filter(
      (p) =>
        p.title.toLowerCase().includes(q) ||
        p.markdown.toLowerCase().includes(q),
    )
  }, [pages, query])

  const filteredToc = useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return toc
    return toc.filter((h) => h.text.toLowerCase().includes(q))
  }, [toc, query])

  return (
    <div className="docs-layout" {...testIdProps(TestIds.docsPage)}>
      <aside className="docs-sidebar" {...testIdProps(TestIds.docsSidebar)}>
        <div className="docs-sidebar-header">
          <h2>{t('docs.title')}</h2>
          <label className="docs-search field">
            <span className="visually-hidden">{t('docs.search')}</span>
            <input
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t('docs.searchPlaceholder')}
              {...testIdProps(TestIds.docsSearch)}
            />
          </label>
        </div>
        <nav aria-label={t('docs.toc')} {...testIdProps(TestIds.docsToc)}>
          <ul className="docs-toc-pages">
            {filteredPages.map((p) => (
              <li key={p.id}>
                <Link
                  to={`/docs/${p.id}`}
                  className={p.id === activeId ? 'active' : undefined}
                  {...testIdProps(`${TestIds.docsTocPagePrefix}${p.id}`)}
                >
                  {p.title}
                </Link>
              </li>
            ))}
          </ul>
          {filteredToc.length > 0 ? (
            <ul className="docs-toc-headings">
              {filteredToc.map((h) => (
                <li key={h.id} className={`docs-toc-h${h.level}`}>
                  <a href={`#${h.id}`} {...testIdProps(TestIds.docsTocHeading)}>
                    {h.text}
                  </a>
                </li>
              ))}
            </ul>
          ) : null}
        </nav>
      </aside>
      <article className="docs-article" {...testIdProps(TestIds.docsArticle)}>
        {page ? (
          <ReactMarkdown
            remarkPlugins={[remarkGfm]}
            components={{
              h2: ({ children }) => {
                const id = headingId(children)
                return <h2 id={id}>{children}</h2>
              },
              h3: ({ children }) => {
                const id = headingId(children)
                return <h3 id={id}>{children}</h3>
              },
              a: ({ href, children }) => {
                if (href?.startsWith('/')) {
                  return (
                    <Link to={href} {...testIdProps(TestIds.docsContentLink)}>
                      {children}
                    </Link>
                  )
                }
                return (
                  <a
                    href={href}
                    rel="noreferrer"
                    {...testIdProps(TestIds.docsContentLink)}
                  >
                    {children}
                  </a>
                )
              },
            }}
          >
            {page.markdown}
          </ReactMarkdown>
        ) : (
          <p className="muted">{t('docs.empty')}</p>
        )}
      </article>
    </div>
  )
}
