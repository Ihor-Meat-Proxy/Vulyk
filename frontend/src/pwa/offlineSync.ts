import { api } from '../api'
import {
  listQueuedStepResults,
  removeQueuedStepResult,
  updateQueuedStepResult,
  type QueuedStepResult,
} from './offlineDb'

export type SyncState = {
  online: boolean
  pending: number
  syncing: boolean
  lastError: string | null
  lastSyncedAt: number | null
}

type Listener = (state: SyncState) => void

const listeners = new Set<Listener>()

let state: SyncState = {
  online: typeof navigator === 'undefined' ? true : navigator.onLine,
  pending: 0,
  syncing: false,
  lastError: null,
  lastSyncedAt: null,
}

function emit() {
  for (const l of listeners) l(state)
}

export function getSyncState(): SyncState {
  return state
}

export function subscribeSync(listener: Listener): () => void {
  listeners.add(listener)
  listener(state)
  return () => {
    listeners.delete(listener)
  }
}

async function refreshPendingCount() {
  try {
    const items = await listQueuedStepResults()
    state = { ...state, pending: items.length }
    emit()
  } catch {
    /* ignore */
  }
}

export async function syncOfflineQueue(): Promise<void> {
  if (typeof navigator !== 'undefined' && !navigator.onLine) {
    state = { ...state, online: false }
    emit()
    return
  }
  if (state.syncing) return

  state = { ...state, syncing: true, online: true, lastError: null }
  emit()

  let items: QueuedStepResult[] = []
  try {
    items = await listQueuedStepResults()
  } catch (err) {
    state = {
      ...state,
      syncing: false,
      lastError: err instanceof Error ? err.message : String(err),
    }
    emit()
    return
  }

  for (const item of items) {
    try {
      await api.createResult(item.projectId, item.runId, item.body)
      await removeQueuedStepResult(item.id)
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      await updateQueuedStepResult({
        ...item,
        attempts: item.attempts + 1,
        lastError: message,
      })
      state = { ...state, lastError: message }
      break
    }
  }

  await refreshPendingCount()
  state = {
    ...state,
    syncing: false,
    lastSyncedAt: Date.now(),
  }
  emit()
}

export function startOfflineSyncWatch(): () => void {
  void refreshPendingCount()

  const onOnline = () => {
    state = { ...state, online: true }
    emit()
    void syncOfflineQueue()
  }
  const onOffline = () => {
    state = { ...state, online: false }
    emit()
  }

  window.addEventListener('online', onOnline)
  window.addEventListener('offline', onOffline)

  if (navigator.onLine) {
    void syncOfflineQueue()
  }

  return () => {
    window.removeEventListener('online', onOnline)
    window.removeEventListener('offline', onOffline)
  }
}

export async function bumpPendingCount(): Promise<void> {
  await refreshPendingCount()
}
