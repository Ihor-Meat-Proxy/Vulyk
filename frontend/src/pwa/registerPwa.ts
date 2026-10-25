import { registerSW } from 'virtual:pwa-register'

export type PwaUpdateApi = {
  needRefresh: boolean
  offlineReady: boolean
  update: () => void
}

type Listener = (api: PwaUpdateApi) => void

const listeners = new Set<Listener>()
let snapshot: PwaUpdateApi = {
  needRefresh: false,
  offlineReady: false,
  update: () => undefined,
}

function emit() {
  for (const l of listeners) l(snapshot)
}

export function subscribePwa(listener: Listener): () => void {
  listeners.add(listener)
  listener(snapshot)
  return () => {
    listeners.delete(listener)
  }
}

export function getPwaState(): PwaUpdateApi {
  return snapshot
}

/** Register the service worker and wire update prompts. */
export function setupPwa(): void {
  if (typeof window === 'undefined') return

  const updateSW = registerSW({
    immediate: true,
    onNeedRefresh() {
      snapshot = {
        ...snapshot,
        needRefresh: true,
        update: () => {
          void updateSW(true)
        },
      }
      emit()
    },
    onOfflineReady() {
      snapshot = { ...snapshot, offlineReady: true }
      emit()
    },
  })

  snapshot = {
    ...snapshot,
    update: () => {
      void updateSW(true)
    },
  }
}
