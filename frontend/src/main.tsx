import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createBrowserRouter, RouterProvider } from 'react-router-dom'
import App from './App'
import { AuthProvider } from './auth'
import { ToastProvider } from './components/ToastProvider'
import { I18nProvider } from './i18n'
import { setupPwa } from './pwa/registerPwa'
import { startOfflineSyncWatch } from './pwa/offlineSync'
import { AppearanceProvider } from './theme/appearance'
import './index.css'

setupPwa()
startOfflineSyncWatch()

const router = createBrowserRouter([
  {
    path: '*',
    element: (
      <AuthProvider>
        <I18nProvider>
          <AppearanceProvider>
            <ToastProvider>
              <App />
            </ToastProvider>
          </AppearanceProvider>
        </I18nProvider>
      </AuthProvider>
    ),
  },
])

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
)
