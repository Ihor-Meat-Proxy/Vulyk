import { useEffect, useId, useState } from 'react'
import { useI18n } from '../i18n'
import { TestIds, testIdProps } from '../testing'
import { Dialog } from './Dialog'

export type FolderDeleteStrategy = 'move_to_parent' | 'delete_cases'

export type FolderDeleteDialogProps = {
  open: boolean
  folderName: string
  canDeleteCases: boolean
  busy?: boolean
  onConfirm: (strategy: FolderDeleteStrategy) => void
  onCancel: () => void
}

export function FolderDeleteDialog({
  open,
  folderName,
  canDeleteCases,
  busy = false,
  onConfirm,
  onCancel,
}: FolderDeleteDialogProps) {
  const { t } = useI18n()
  const groupId = useId()
  const [strategy, setStrategy] =
    useState<FolderDeleteStrategy>('move_to_parent')

  useEffect(() => {
    if (open) setStrategy('move_to_parent')
  }, [open])

  return (
    <Dialog
      open={open}
      title={t('folders.delete')}
      description={t('folders.deleteNonemptyHelp').replace(
        '{name}',
        folderName,
      )}
      onClose={onCancel}
      asForm
      onSubmit={() => {
        if (!busy) onConfirm(strategy)
      }}
      panelTestId={TestIds.folderDeleteDialogPanel}
      footer={
        <>
          <button
            type="button"
            className="btn secondary"
            onClick={onCancel}
            disabled={busy}
            {...testIdProps(TestIds.folderDeleteDialogCancel)}
          >
            {t('common.cancel')}
          </button>
          <button
            type="submit"
            className="btn danger"
            disabled={busy || (strategy === 'delete_cases' && !canDeleteCases)}
            {...testIdProps(TestIds.folderDeleteDialogConfirm)}
          >
            {t('common.delete')}
          </button>
        </>
      }
    >
      <fieldset
        className="app-dialog-fieldset"
        {...testIdProps(TestIds.folderDeleteDialogOptions)}
      >
        <legend className="sr-only">{t('folders.deleteStrategy')}</legend>
        <label className="app-dialog-radio">
          <input
            type="radio"
            name={groupId}
            value="move_to_parent"
            checked={strategy === 'move_to_parent'}
            onChange={() => setStrategy('move_to_parent')}
            {...testIdProps(TestIds.folderDeleteDialogMoveOption)}
          />
          <span>{t('folders.deleteMoveToParent')}</span>
        </label>
        <label className="app-dialog-radio">
          <input
            type="radio"
            name={groupId}
            value="delete_cases"
            checked={strategy === 'delete_cases'}
            disabled={!canDeleteCases}
            onChange={() => setStrategy('delete_cases')}
            {...testIdProps(TestIds.folderDeleteDialogDeleteCasesOption)}
          />
          <span>{t('folders.deleteCases')}</span>
        </label>
        {!canDeleteCases ? (
          <p className="muted app-dialog-hint">
            {t('folders.deleteCasesForbidden')}
          </p>
        ) : null}
      </fieldset>
    </Dialog>
  )
}
