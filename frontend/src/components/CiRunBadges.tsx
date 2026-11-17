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

type CiKind =
  'github' | 'gitlab' | 'jenkins' | 'forgejo' | 'gitea' | 'woodpecker' | 'ci'

function normalizeProvider(
  source?: string | null,
  ciProvider?: string | null,
): CiKind | null {
  const provider = (ciProvider ?? '').toLowerCase()
  if (
    provider === 'github' ||
    provider === 'gitlab' ||
    provider === 'jenkins' ||
    provider === 'forgejo' ||
    provider === 'gitea' ||
    provider === 'woodpecker'
  ) {
    return provider
  }
  const src = (source ?? '').toLowerCase()
  if (src === 'ci:github' || src.includes('github')) return 'github'
  if (src === 'ci:gitlab' || src.includes('gitlab')) return 'gitlab'
  if (src === 'ci:jenkins' || src.includes('jenkins')) return 'jenkins'
  if (src === 'ci:forgejo' || src.includes('forgejo')) return 'forgejo'
  if (src === 'ci:gitea' || src.includes('gitea')) return 'gitea'
  if (src === 'ci:woodpecker' || src.includes('woodpecker')) return 'woodpecker'
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
        : kind === 'jenkins'
          ? t('executions.sourceJenkins')
          : kind === 'forgejo'
            ? t('executions.sourceForgejo')
            : kind === 'gitea'
              ? t('executions.sourceGitea')
              : kind === 'woodpecker'
                ? t('executions.sourceWoodpecker')
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
