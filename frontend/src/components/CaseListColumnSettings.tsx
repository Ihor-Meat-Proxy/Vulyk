import { useEffect, useId, useRef, useState } from 'react'
import type { CaseListColumnsConfig, ResolvedColumn } from '../caseListColumns'
import { moveColumn, setColumnVisible } from '../caseListColumns'
import { useFocusTrap } from '../hooks/useFocusTrap'
import { useI18n } from '../i18n'
import { TestIds, listItemProps, testIdProps } from '../testing'

type Props = {
  config: CaseListColumnsConfig
  registry: ResolvedColumn[]
  onChange: (next: CaseListColumnsConfig) => void
  onReset: () => void
}

export function CaseListColumnSettings({
  config,
  registry,
  onChange,
  onReset,
}: Props) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const panelId = useId()
  const rootRef = useRef<HTMLDivElement>(null)
  const dialogRef = useRef<HTMLDivElement>(null)
  const close = () => setOpen(false)

  useFocusTrap(dialogRef, open, { onEscape: close })

  useEffect(() => {
    if (!open) return
    const onDoc = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', onDoc)
    return () => {
      document.removeEventListener('mousedown', onDoc)
    }
  }, [open])

  const labelFor = (col: ResolvedColumn) =>
    col.label ?? (col.labelKey ? t(col.labelKey) : col.id)

  return (
    <div className="case-col-settings" ref={rootRef}>
      <button
        type="button"
        className="btn secondary case-col-settings-toggle"
        aria-expanded={open}
        aria-controls={panelId}
        aria-haspopup="dialog"
        onClick={() => setOpen((v) => !v)}
        {...testIdProps(TestIds.caseListColumnsButton)}
      >
        {t('cases.columns')}
      </button>
      {open && (
        <div
          ref={dialogRef}
          id={panelId}
          role="dialog"
          aria-modal="true"
          aria-label={t('cases.columnsSettings')}
          className="case-col-settings-panel"
          {...testIdProps(TestIds.caseListColumnsMenu)}
        >
          <p className="muted case-col-settings-hint">
            {t('cases.columnsHint')}
          </p>
          <ul className="case-col-settings-list">
            {config.columns.map((pref, index) => {
              const def = registry.find((r) => r.id === pref.id)
              if (!def) return null
              const label = labelFor(def)
              return (
                <li
                  key={pref.id}
                  className="case-col-settings-row"
                  {...listItemProps(TestIds.caseListColumnItem, {
                    entityId: pref.id,
                    entityKey: pref.id,
                    index,
                  })}
                >
                  <label className="case-col-settings-check">
                    <input
                      type="checkbox"
                      checked={pref.visible}
                      disabled={!def.hideable}
                      onChange={(e) =>
                        onChange(
                          setColumnVisible(
                            config,
                            pref.id,
                            e.target.checked,
                            registry,
                          ),
                        )
                      }
                      aria-label={t('cases.columnToggle', { column: label })}
                      {...testIdProps(TestIds.caseListColumnItemCheckbox)}
                    />
                    <span>{label}</span>
                  </label>
                  <div className="case-col-settings-move">
                    <button
                      type="button"
                      className="btn secondary btn-icon"
                      disabled={index === 0}
                      aria-label={t('cases.columnMoveLeft', { column: label })}
                      onClick={() =>
                        onChange(moveColumn(config, pref.id, 'left'))
                      }
                      {...testIdProps(TestIds.caseListColumnItemMoveLeft)}
                    >
                      ←
                    </button>
                    <button
                      type="button"
                      className="btn secondary btn-icon"
                      disabled={index === config.columns.length - 1}
                      aria-label={t('cases.columnMoveRight', {
                        column: label,
                      })}
                      onClick={() =>
                        onChange(moveColumn(config, pref.id, 'right'))
                      }
                      {...testIdProps(TestIds.caseListColumnItemMoveRight)}
                    >
                      →
                    </button>
                  </div>
                </li>
              )
            })}
          </ul>
          <button
            type="button"
            className="btn secondary case-col-reset"
            onClick={() => {
              onReset()
              setOpen(false)
            }}
            {...testIdProps(TestIds.caseListColumnsResetButton)}
          >
            {t('cases.columnsReset')}
          </button>
        </div>
      )}
    </div>
  )
}
