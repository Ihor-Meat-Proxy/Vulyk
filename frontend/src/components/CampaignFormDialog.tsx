import { useEffect, useId, useMemo, useState } from 'react'
import { ApiError } from '../api'
import { useI18n } from '../i18n'
import { TestIds, testIdProps } from '../testing'
import { Dialog } from './Dialog'

export type CampaignFormMode = 'campaign' | 'iteration'

export type CampaignFormValues = {
  name: string
  description: string
  cadence?: string
  mode?: string
}

export type CampaignFormDialogProps = {
  open: boolean
  mode: CampaignFormMode
  /** Existing names in the same scope (project campaigns, or iterations under a campaign). */
  existingNames: string[]
  busy?: boolean
  apiError?: string | null
  onSubmit: (values: CampaignFormValues) => void | Promise<void>
  onCancel: () => void
}

export function CampaignFormDialog({
  open,
  mode,
  existingNames,
  busy = false,
  apiError = null,
  onSubmit,
  onCancel,
}: CampaignFormDialogProps) {
  const { t } = useI18n()
  const nameId = useId()
  const descId = useId()
  const cadenceId = useId()
  const modeId = useId()
  const errorId = useId()

  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [cadence, setCadence] = useState('manual')
  const [iterationMode, setIterationMode] = useState('automated')
  const [error, setError] = useState<string | null>(null)

  const existingLower = useMemo(
    () => new Set(existingNames.map((n) => n.trim().toLowerCase())),
    [existingNames],
  )

  useEffect(() => {
    if (open) {
      setName('')
      setDescription('')
      setCadence('manual')
      setIterationMode('automated')
      setError(null)
    }
  }, [open])

  useEffect(() => {
    if (apiError) setError(apiError)
  }, [apiError])

  const title =
    mode === 'campaign'
      ? t('executions.newCampaign')
      : t('executions.newIteration')
  const nameLabel =
    mode === 'campaign'
      ? t('executions.campaignName')
      : t('executions.iterationName')

  const nameTestId =
    mode === 'campaign'
      ? TestIds.campaignDialogNameInput
      : TestIds.iterationDialogNameInput
  const descTestId =
    mode === 'campaign'
      ? TestIds.campaignDialogDescriptionInput
      : TestIds.iterationDialogDescriptionInput
  const submitTestId =
    mode === 'campaign'
      ? TestIds.campaignDialogSubmit
      : TestIds.iterationDialogSubmit
  const cancelTestId =
    mode === 'campaign'
      ? TestIds.campaignDialogCancel
      : TestIds.iterationDialogCancel
  const errorTestId =
    mode === 'campaign'
      ? TestIds.campaignDialogError
      : TestIds.iterationDialogError

  const submit = () => {
    const trimmed = name.trim()
    if (!trimmed) {
      setError(t('dialog.nameRequired'))
      return
    }
    if (existingLower.has(trimmed.toLowerCase())) {
      setError(t('dialog.nameTaken'))
      return
    }
    setError(null)
    void Promise.resolve(
      onSubmit({
        name: trimmed,
        description: description.trim(),
        ...(mode === 'iteration'
          ? { cadence: cadence.trim() || 'manual', mode: iterationMode }
          : {}),
      }),
    ).catch((err: unknown) => {
      if (err instanceof ApiError && err.status === 409) {
        setError(t('dialog.nameTaken'))
        return
      }
      setError(err instanceof Error ? err.message : String(err))
    })
  }

  return (
    <Dialog
      open={open}
      title={title}
      onClose={onCancel}
      asForm
      onSubmit={submit}
      panelTestId={
        mode === 'campaign'
          ? TestIds.campaignDialogPanel
          : TestIds.iterationDialogPanel
      }
      footer={
        <>
          <button
            type="button"
            className="btn secondary"
            onClick={onCancel}
            disabled={busy}
            {...testIdProps(cancelTestId)}
          >
            {t('common.cancel')}
          </button>
          <button
            type="submit"
            className="btn"
            disabled={busy}
            {...testIdProps(submitTestId)}
          >
            {t('common.create')}
          </button>
        </>
      }
    >
      <label className="app-dialog-field" htmlFor={nameId}>
        <span>{nameLabel}</span>
        <input
          id={nameId}
          type="text"
          value={name}
          required
          autoComplete="off"
          aria-invalid={error ? true : undefined}
          aria-describedby={error ? errorId : undefined}
          onChange={(e) => {
            setName(e.target.value)
            if (error) setError(null)
          }}
          {...testIdProps(nameTestId)}
        />
      </label>
      <label className="app-dialog-field" htmlFor={descId}>
        <span>{t('dialog.descriptionOptional')}</span>
        <textarea
          id={descId}
          value={description}
          rows={2}
          onChange={(e) => setDescription(e.target.value)}
          {...testIdProps(descTestId)}
        />
      </label>
      {mode === 'iteration' ? (
        <>
          <label className="app-dialog-field" htmlFor={cadenceId}>
            <span>{t('executions.cadence')}</span>
            <input
              id={cadenceId}
              type="text"
              value={cadence}
              placeholder={t('executions.cadencePlaceholder')}
              onChange={(e) => setCadence(e.target.value)}
              {...testIdProps(TestIds.iterationDialogCadenceInput)}
            />
          </label>
          <label className="app-dialog-field" htmlFor={modeId}>
            <span>{t('executions.mode')}</span>
            <select
              id={modeId}
              value={iterationMode}
              onChange={(e) => setIterationMode(e.target.value)}
              {...testIdProps(TestIds.iterationDialogModeSelect)}
            >
              <option value="automated">{t('executions.modeAutomated')}</option>
              <option value="manual">{t('executions.modeManual')}</option>
              <option value="mixed">{t('executions.modeMixed')}</option>
            </select>
          </label>
        </>
      ) : null}
      {error ? (
        <p
          id={errorId}
          className="error app-dialog-error"
          role="alert"
          {...testIdProps(errorTestId)}
        >
          {error}
        </p>
      ) : null}
    </Dialog>
  )
}
