import { useEffect, useState } from 'react'
import {
  getPwaState,
  subscribePwa,
  type PwaUpdateApi,
} from '../pwa/registerPwa'
import {
  getSyncState,
  subscribeSync,
  syncOfflineQueue,
  type SyncState,
} from '../pwa/offlineSync'
import { useI18n } from '../i18n'
import { TestIds, testIdProps } from '../testing'

type BeforeInstallPromptEvent = Event & {
  prompt: () => Promise<void>
  userChoice: Promise<{ outcome: 'accepted' | 'dismissed' }>
}

const INSTALL_DISMISS_KEY = 'vulyk.pwa.installDismissed'

export function PwaToasts() {
  const { t } = useI18n()
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
          <span>{t('pwa.offline')}</span>
        </div>
      )}
      {showPending && (
        <div className="pwa-toast" {...testIdProps(TestIds.pwaToastPending)}>
          <span>
            {t('pwa.queued', { count: sync.pending })}
            {sync.syncing
              ? ` ${t('pwa.syncing')}`
              : sync.online
                ? ''
                : ` ${t('pwa.waitingNetwork')}`}
          </span>
          {sync.online && !sync.syncing && (
            <button
              type="button"
              className="btn secondary"
              onClick={() => void syncOfflineQueue()}
              {...testIdProps(TestIds.pwaSyncNowButton)}
            >
              {t('pwa.syncNow')}
            </button>
          )}
        </div>
      )}
      {showUpdate && (
        <div
          className="pwa-toast pwa-toast-accent"
          {...testIdProps(TestIds.pwaToastUpdate)}
        >
          <span>{t('pwa.updateReady')}</span>
          <button
            type="button"
            className="btn"
            onClick={() => pwa.update()}
            {...testIdProps(TestIds.pwaUpdateButton)}
          >
            {t('pwa.update')}
          </button>
        </div>
      )}
      {showOfflineReady && (
        <div
          className="pwa-toast"
          {...testIdProps(TestIds.pwaToastOfflineReady)}
        >
          <span>{t('pwa.offlineReady')}</span>
          <button
            type="button"
            className="btn secondary"
            onClick={() => setOfflineReadyAck(true)}
            {...testIdProps(TestIds.pwaDismissButton)}
          >
            {t('pwa.dismiss')}
          </button>
        </div>
      )}
      {showInstall && (
        <div
          className="pwa-toast pwa-toast-accent"
          {...testIdProps(TestIds.pwaToastInstall)}
        >
          <span>{t('pwa.installPrompt')}</span>
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
            {t('pwa.install')}
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
            {t('pwa.notNow')}
          </button>
        </div>
      )}
    </div>
  )
}
