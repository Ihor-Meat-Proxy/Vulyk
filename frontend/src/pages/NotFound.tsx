import { Link } from 'react-router-dom'
import { useI18n } from '../i18n'
import { TestIds, testIdProps } from '../testing'

export function NotFound() {
  const { t } = useI18n()

  return (
    <div
      className="panel not-found-page"
      {...testIdProps(TestIds.notFoundPage)}
    >
      <h1 style={{ marginTop: 0 }} {...testIdProps(TestIds.notFoundTitle)}>
        {t('notFound.title')}
      </h1>
      <p className="muted">{t('notFound.body')}</p>
      <p>
        <Link to="/" className="btn" {...testIdProps(TestIds.notFoundBackLink)}>
          {t('notFound.back')}
        </Link>
      </p>
    </div>
  )
}
