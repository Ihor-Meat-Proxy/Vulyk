import { Link } from 'react-router-dom'
import { useI18n } from '../i18n'
import { TestIds, testIdProps } from '../testing'

type Props = {
  /** In-app docs path, e.g. `/docs/runs`. */
  to: string
  /** Suffix for `docs-guide-link-<suffix>` test id. */
  testIdSuffix: string
  className?: string
}

/** Shared “Detailed guide” link into the offline `/docs` viewer. */
export function DocsLink({ to, testIdSuffix, className }: Props) {
  const { t } = useI18n()
  return (
    <Link
      to={to}
      className={className ?? 'docs-guide-link'}
      {...testIdProps(`${TestIds.docsGuideLink}-${testIdSuffix}`)}
    >
      {t('docs.detailedGuide')}
    </Link>
  )
}
