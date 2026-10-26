import { useI18n } from '../i18n'

type Props = {
  className?: string
  testId?: string
}

/** Small chip marking seeded demo/sample entities. */
export function SampleBadge({ className, testId }: Props) {
  const { t } = useI18n()
  return (
    <span
      className={`sample-badge${className ? ` ${className}` : ''}`}
      data-testid={testId ?? 'sample-badge'}
    >
      {t('common.sample')}
    </span>
  )
}
