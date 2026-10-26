import { useI18n } from '../i18n'
import { TestIds, testIdProps } from '../testing'

type SourceProps = {
  source?: string | null
  ciProvider?: string | null
}

type PipelineProps = {
  pipelineUrl?: string | null
  jobUrl?: string | null
}

function normalizeProvider(
  source?: string | null,
  ciProvider?: string | null,
): 'github' | 'gitlab' | 'ci' | null {
  const provider = (ciProvider ?? '').toLowerCase()
  if (provider === 'github' || provider === 'gitlab') return provider
  const src = (source ?? '').toLowerCase()
  if (src === 'ci:github' || src.includes('github')) return 'github'
  if (src === 'ci:gitlab' || src.includes('gitlab')) return 'gitlab'
  if (src === 'ci' || src.startsWith('ci:') || provider === 'other') return 'ci'
  return null
}

export function CiSourceBadge({ source, ciProvider }: SourceProps) {
  const { t } = useI18n()
  const kind = normalizeProvider(source, ciProvider)
  if (!kind) return null
  const label =
    kind === 'github'
      ? t('executions.sourceGithub')
      : kind === 'gitlab'
        ? t('executions.sourceGitlab')
        : t('executions.sourceCi')
  return <span className="badge">{label}</span>
}

export function CiPipelineLink({ pipelineUrl, jobUrl }: PipelineProps) {
  const { t } = useI18n()
  const href = pipelineUrl || jobUrl
  if (!href) return null
  return (
    <a
      className="linkish"
      href={href}
      target="_blank"
      rel="noreferrer"
      {...testIdProps(TestIds.executionsPipelineLink)}
    >
      {t('executions.pipelineLink')}
    </a>
  )
}
