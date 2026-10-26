import { useCallback, useEffect, useState, type FormEvent } from 'react'
import {
  api,
  type OptionList,
  type OptionListDetail,
  type OptionListItem,
} from '../../api'
import { useI18n } from '../../i18n'

type Props = {
  onError: (msg: string) => void
}

export function ListsPanel({ onError }: Props) {
  const { t } = useI18n()
  const [lists, setLists] = useState<OptionList[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [detail, setDetail] = useState<OptionListDetail | null>(null)
  const [key, setKey] = useState('')
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [itemLabel, setItemLabel] = useState('')
  const [renameName, setRenameName] = useState('')
  const [busy, setBusy] = useState(false)

  const loadLists = useCallback(async () => {
    try {
      setLists(await api.listAdminLists())
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    }
  }, [onError])

  const loadDetail = useCallback(
    async (id: string) => {
      try {
        const d = await api.getAdminList(id)
        setDetail(d)
        setRenameName(d.name)
      } catch (err) {
        onError(err instanceof Error ? err.message : String(err))
      }
    },
    [onError],
  )

  useEffect(() => {
    void loadLists()
  }, [loadLists])

  useEffect(() => {
    if (selectedId) void loadDetail(selectedId)
    else setDetail(null)
  }, [selectedId, loadDetail])

  const onCreate = async (e: FormEvent) => {
    e.preventDefault()
    setBusy(true)
    try {
      const created = await api.createAdminList({
        key: key.trim(),
        name: name.trim(),
        description: description.trim(),
      })
      setKey('')
      setName('')
      setDescription('')
      await loadLists()
      setSelectedId(created.id)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const onRename = async (e: FormEvent) => {
    e.preventDefault()
    if (!detail) return
    setBusy(true)
    try {
      await api.updateAdminList(detail.id, { name: renameName.trim() })
      await loadLists()
      await loadDetail(detail.id)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const onAddItem = async (e: FormEvent) => {
    e.preventDefault()
    if (!detail || !itemLabel.trim()) return
    setBusy(true)
    try {
      await api.createAdminListItem(detail.id, {
        label: itemLabel.trim(),
        sort_order: detail.items.length,
      })
      setItemLabel('')
      await loadDetail(detail.id)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const moveItem = async (item: OptionListItem, dir: -1 | 1) => {
    if (!detail) return
    const ids = detail.items.map((i) => i.id)
    const idx = ids.indexOf(item.id)
    const swap = idx + dir
    if (idx < 0 || swap < 0 || swap >= ids.length) return
    ;[ids[idx], ids[swap]] = [ids[swap], ids[idx]]
    setBusy(true)
    try {
      await api.reorderAdminListItems(detail.id, ids)
      await loadDetail(detail.id)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const deprecateItem = async (item: OptionListItem) => {
    if (!detail) return
    setBusy(true)
    try {
      await api.updateAdminListItem(detail.id, item.id, {
        deprecated: !item.deprecated,
      })
      await loadDetail(detail.id)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const deleteItem = async (item: OptionListItem) => {
    if (!detail) return
    if (!window.confirm(t('admin.lists.deleteItemConfirm'))) return
    setBusy(true)
    try {
      await api.deleteAdminListItem(detail.id, item.id)
      await loadDetail(detail.id)
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="panel admin-grid">
      <div>
        <h3>{t('admin.lists')}</h3>
        <p className="muted">{t('admin.lists.help')}</p>
        <table className="table">
          <thead>
            <tr>
              <th>{t('admin.lists.key')}</th>
              <th>{t('admin.lists.name')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {lists.map((l) => (
              <tr key={l.id}>
                <td>
                  <span className="badge">{l.key}</span>
                </td>
                <td>{l.name}</td>
                <td>
                  <button
                    type="button"
                    className="btn secondary"
                    onClick={() => setSelectedId(l.id)}
                  >
                    {t('admin.lists.open')}
                  </button>
                </td>
              </tr>
            ))}
            {lists.length === 0 && (
              <tr>
                <td colSpan={3} className="muted">
                  {t('admin.lists.empty')}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="stack-form">
        <form onSubmit={(e) => void onCreate(e)}>
          <h3>{t('admin.lists.create')}</h3>
          <label className="field">
            <span>{t('admin.lists.key')}</span>
            <input
              value={key}
              onChange={(e) => setKey(e.target.value)}
              required
              pattern="[a-z0-9_\\-]+"
            />
          </label>
          <label className="field">
            <span>{t('admin.lists.name')}</span>
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              required
            />
          </label>
          <label className="field">
            <span>{t('admin.lists.description')}</span>
            <textarea
              className="area"
              rows={2}
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </label>
          <button className="btn" type="submit" disabled={busy}>
            {t('admin.lists.create')}
          </button>
        </form>

        {detail && (
          <>
            <hr />
            <h3>
              {detail.name}{' '}
              <span className="muted badge">{detail.key}</span>
            </h3>
            <form onSubmit={(e) => void onRename(e)}>
              <label className="field">
                <span>{t('admin.lists.rename')}</span>
                <input
                  value={renameName}
                  onChange={(e) => setRenameName(e.target.value)}
                  required
                />
              </label>
              <button className="btn secondary" type="submit" disabled={busy}>
                {t('common.save')}
              </button>
            </form>

            <table className="table">
              <thead>
                <tr>
                  <th>{t('admin.lists.itemLabel')}</th>
                  <th>{t('admin.lists.itemStatus')}</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {detail.items.map((item, idx) => (
                  <tr key={item.id}>
                    <td>
                      {item.label}
                      {item.value && item.value !== item.label ? (
                        <span className="muted"> ({item.value})</span>
                      ) : null}
                    </td>
                    <td>
                      {item.deprecated
                        ? t('admin.lists.deprecated')
                        : item.active
                          ? t('admin.lists.active')
                          : t('admin.lists.inactive')}
                    </td>
                    <td className="row-actions">
                      <button
                        type="button"
                        className="btn secondary"
                        disabled={busy || idx === 0}
                        onClick={() => void moveItem(item, -1)}
                      >
                        ↑
                      </button>
                      <button
                        type="button"
                        className="btn secondary"
                        disabled={busy || idx === detail.items.length - 1}
                        onClick={() => void moveItem(item, 1)}
                      >
                        ↓
                      </button>
                      <button
                        type="button"
                        className="btn secondary"
                        disabled={busy}
                        onClick={() => void deprecateItem(item)}
                      >
                        {item.deprecated
                          ? t('admin.lists.undeprecate')
                          : t('admin.lists.deprecate')}
                      </button>
                      <button
                        type="button"
                        className="btn secondary"
                        disabled={busy}
                        onClick={() => void deleteItem(item)}
                      >
                        {t('common.delete')}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>

            <form onSubmit={(e) => void onAddItem(e)}>
              <label className="field">
                <span>{t('admin.lists.addItem')}</span>
                <input
                  value={itemLabel}
                  onChange={(e) => setItemLabel(e.target.value)}
                  required
                />
              </label>
              <button className="btn" type="submit" disabled={busy}>
                {t('admin.lists.addItem')}
              </button>
            </form>
          </>
        )}
      </div>
    </div>
  )
}
