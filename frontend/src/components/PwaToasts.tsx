import { useEffect, useState } from 'react'
import { getPwaState, subscribePwa, type PwaUpdateApi } from '../pwa/registerPwa'
import {
  getSyncState,
  subscribeSync,
  syncOfflineQueue,
  type SyncState,
} from '../pwa/offlineSync'
import { TestIds, testIdProps } from '../testing'

type BeforeInstallPromptEvent = Event & {
  prompt: () => Promise<void>
  userChoice: Promise<{ outcome: 'accepted' | 'dismissed' }>
}

const INSTALL_DISMISS_KEY = 'vulyk.pwa.installDismissed'

export function PwaToasts() {
  const [pwa, setPwa] = useState<PwaUpdateApi>(getPwaState)
  const [sync, setSync] = useState<SyncState>(getSyncState)
  const [installEvent, setInstallEvent] =
    useState<BeforeInstallPromptEvent | null>(null)
  const [installDismissed, setInstallDismissed] = useState(() => {
    try {
      return localStorage.getItem(INSTALL_DISMISS_KEY) === '1'
    } catch {
      return false
    }
  })
  const [offlineReadyAck, setOfflineReadyAck] = useState(false)

  useEffect(() => subscribePwa(setPwa), [])
  useEffect(() => subscribeSync(setSync), [])

  useEffect(() => {
    const onBip = (e: Event) => {
      e.preventDefault()
      setInstallEvent(e as BeforeInstallPromptEvent)
    }
    window.addEventListener('beforeinstallprompt', onBip)
    return () => window.removeEventListener('beforeinstallprompt', onBip)
  }, [])

  const showInstall = Boolean(installEvent) && !installDismissed
  const showUpdate = pwa.needRefresh
  const showOfflineReady = pwa.offlineReady && !offlineReadyAck
  const showOffline = !sync.online
  const showPending = sync.pending > 0

  if (
    !showInstall &&
    !showUpdate &&
    !showOfflineReady &&
    !showOffline &&
    !showPending
  ) {
    return null
  }

  return (
    <div
      className="pwa-toasts"
      role="status"
      aria-live="polite"
      {...testIdProps(TestIds.pwaToasts)}
    >
      {showOffline && (
        <div
          className="pwa-toast pwa-toast-warn"
          {...testIdProps(TestIds.pwaToastOffline)}
        >
          <span>Offline — showing cached cases/runs when available.</span>
        </div>
      )}
      {showPending && (
        <div className="pwa-toast" {...testIdProps(TestIds.pwaToastPending)}>
          <span>
            {sync.pending} result{sync.pending === 1 ? '' : 's'} queued
            {sync.syncing ? ' — syncing…' : sync.online ? '' : ' (waiting for network)'}
          </span>
          {sync.online && !sync.syncing && (
            <button
              type="button"
              className="btn secondary"
              onClick={() => void syncOfflineQueue()}
              {...testIdProps(TestIds.pwaSyncNowButton)}
            >
              Sync now
            </button>
          )}
        </div>
      )}
      {showUpdate && (
        <div
          className="pwa-toast pwa-toast-accent"
          {...testIdProps(TestIds.pwaToastUpdate)}
        >
          <span>A new Vulyk version is ready.</span>
          <button
            type="button"
            className="btn"
            onClick={() => pwa.update()}
            {...testIdProps(TestIds.pwaUpdateButton)}
          >
            Update
          </button>
        </div>
      )}
      {showOfflineReady && (
        <div
          className="pwa-toast"
          {...testIdProps(TestIds.pwaToastOfflineReady)}
        >
          <span>App shell cached — ready for offline use.</span>
          <button
            type="button"
            className="btn secondary"
            onClick={() => setOfflineReadyAck(true)}
            {...testIdProps(TestIds.pwaDismissButton)}
          >
            Dismiss
          </button>
        </div>
      )}
      {showInstall && (
        <div
          className="pwa-toast pwa-toast-accent"
          {...testIdProps(TestIds.pwaToastInstall)}
        >
          <span>Install Vulyk for quick access on this device.</span>
          <button
            type="button"
            className="btn"
            onClick={() => {
              void (async () => {
                if (!installEvent) return
                await installEvent.prompt()
                await installEvent.userChoice
                setInstallEvent(null)
              })()
            }}
            {...testIdProps(TestIds.pwaInstallButton)}
          >
            Install
          </button>
          <button
            type="button"
            className="btn secondary"
            onClick={() => {
              try {
                localStorage.setItem(INSTALL_DISMISS_KEY, '1')
              } catch {
                /* ignore */
              }
              setInstallDismissed(true)
            }}
            {...testIdProps(TestIds.pwaInstallDismissButton)}
          >
            Not now
          </button>
        </div>
      )}
    </div>
  )
}
