import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react'
import { api } from '../api'
import { useAuth } from '../auth'

export type Locale = 'en' | 'uk'

const STORAGE_KEY = 'vulyk.locale'

type Dict = Record<string, string>

const en: Dict = {
  'nav.cases': 'Cases',
  'nav.requirements': 'Requirements',
  'nav.import': 'Import',
  'nav.generate': 'Generate',
  'nav.chat': 'Chat',
  'nav.runs': 'Runs',
  'nav.ci': 'CI',
  'nav.reports': 'Reports',
  'nav.admin': 'Admin',
  'nav.suites': 'Suites',
  'nav.allCases': 'All cases',
  'nav.close': 'Close',
  'nav.project': 'Project',
  'nav.signOut': 'Sign out',
  'nav.openMode': 'Open mode',
  'nav.locale': 'Language',
  'login.title': 'Sign in',
  'login.bootstrapTitle': 'Create the first admin',
  'login.subtitle':
    'Use your local account. Personal API tokens for CI/MCP are managed under Admin → Tokens after sign-in.',
  'login.bootstrapSubtitle':
    'No users yet. Bootstrap a system administrator to unlock the admin panel and API tokens.',
  'login.email': 'Email',
  'login.password': 'Password',
  'login.displayName': 'Display name',
  'login.submit': 'Sign in',
  'login.createAdmin': 'Create admin',
  'login.register': 'Create an account',
  'login.registerTitle': 'Register',
  'login.registerSubmit': 'Register',
  'login.backToSignIn': 'Back to sign in',
  'login.sso': 'Continue with {name}',
  'login.working': 'Working…',
  'setup.title': 'Instance setup',
  'setup.subtitle':
    'Complete first-time configuration with the setup token from your host environment.',
  'setup.token': 'Setup token',
  'setup.adminEmail': 'Admin email',
  'setup.adminPassword': 'Admin password',
  'setup.passwordHint': 'At least 8 characters with a letter and a digit.',
  'setup.displayName': 'Display name',
  'setup.instanceName': 'Instance name',
  'setup.baseUrl': 'Base URL',
  'setup.locale': 'Default locale',
  'setup.timezone': 'Default timezone (IANA)',
  'setup.registrationMode': 'Registration mode',
  'setup.modeOpen': 'Open',
  'setup.modeInvite': 'Invite only',
  'setup.modeDisabled': 'Disabled',
  'setup.submit': 'Complete setup',
  'setup.success': 'Setup complete. Sign in with the admin account.',
  'password.title': 'Change password',
  'password.subtitle': 'You must set a new password before continuing.',
  'password.current': 'Current password',
  'password.new': 'New password',
  'password.confirm': 'Confirm new password',
  'password.submit': 'Update password',
  'password.mismatch': 'Passwords do not match.',
  'admin.title': 'Admin',
  'admin.subtitle':
    'Users, project roles, integrations, LLM providers, and API tokens for CI/MCP.',
  'admin.users': 'Users',
  'admin.roles': 'Roles',
  'admin.projects': 'Projects',
  'admin.integrations': 'Integrations',
  'admin.llm': 'LLM providers',
  'admin.tokens': 'API tokens',
  'admin.settings': 'Settings',
  'admin.sso': 'SSO',
  'common.loading': 'Loading…',
  'common.save': 'Save',
  'common.cancel': 'Cancel',
}

const uk: Dict = {
  'nav.cases': 'Кейси',
  'nav.requirements': 'Вимоги',
  'nav.import': 'Імпорт',
  'nav.generate': 'Генерація',
  'nav.chat': 'Чат',
  'nav.runs': 'Прогони',
  'nav.ci': 'CI',
  'nav.reports': 'Звіти',
  'nav.admin': 'Адмін',
  'nav.suites': 'Сьюти',
  'nav.allCases': 'Усі кейси',
  'nav.close': 'Закрити',
  'nav.project': 'Проєкт',
  'nav.signOut': 'Вийти',
  'nav.openMode': 'Відкритий режим',
  'nav.locale': 'Мова',
  'login.title': 'Увійти',
  'login.bootstrapTitle': 'Створити першого адміна',
  'login.subtitle':
    'Увійдіть локальним обліковим записом. Токени API для CI/MCP — у Адмін → Токени.',
  'login.bootstrapSubtitle':
    'Користувачів ще немає. Створіть системного адміністратора, щоб відкрити адмінку та токени API.',
  'login.email': 'Email',
  'login.password': 'Пароль',
  'login.displayName': "Ім'я",
  'login.submit': 'Увійти',
  'login.createAdmin': 'Створити адміна',
  'login.register': 'Створити обліковий запис',
  'login.registerTitle': 'Реєстрація',
  'login.registerSubmit': 'Зареєструватися',
  'login.backToSignIn': 'Назад до входу',
  'login.sso': 'Увійти через {name}',
  'login.working': 'Зачекайте…',
  'setup.title': 'Налаштування інстансу',
  'setup.subtitle':
    'Завершіть первинне налаштування за допомогою setup-токена з середовища хоста.',
  'setup.token': 'Setup-токен',
  'setup.adminEmail': 'Email адміна',
  'setup.adminPassword': 'Пароль адміна',
  'setup.passwordHint': 'Мінімум 8 символів, літера та цифра.',
  'setup.displayName': "Ім'я",
  'setup.instanceName': 'Назва інстансу',
  'setup.baseUrl': 'Базовий URL',
  'setup.locale': 'Мова за замовчуванням',
  'setup.timezone': 'Часовий пояс (IANA)',
  'setup.registrationMode': 'Режим реєстрації',
  'setup.modeOpen': 'Відкритий',
  'setup.modeInvite': 'Лише за запрошенням',
  'setup.modeDisabled': 'Вимкнено',
  'setup.submit': 'Завершити налаштування',
  'setup.success': 'Готово. Увійдіть обліковим записом адміна.',
  'password.title': 'Зміна пароля',
  'password.subtitle': 'Потрібно встановити новий пароль, перш ніж продовжити.',
  'password.current': 'Поточний пароль',
  'password.new': 'Новий пароль',
  'password.confirm': 'Підтвердіть новий пароль',
  'password.submit': 'Оновити пароль',
  'password.mismatch': 'Паролі не збігаються.',
  'admin.title': 'Адмін',
  'admin.subtitle':
    'Користувачі, ролі проєкту, інтеграції, LLM і токени API для CI/MCP.',
  'admin.users': 'Користувачі',
  'admin.roles': 'Ролі',
  'admin.projects': 'Проєкти',
  'admin.integrations': 'Інтеграції',
  'admin.llm': 'LLM',
  'admin.tokens': 'Токени API',
  'admin.settings': 'Налаштування',
  'admin.sso': 'SSO',
  'common.loading': 'Завантаження…',
  'common.save': 'Зберегти',
  'common.cancel': 'Скасувати',
}

const dictionaries: Record<Locale, Dict> = { en, uk }

function normalizeLocale(value: string | null | undefined): Locale | null {
  if (!value) return null
  const base = value.toLowerCase().split(/[-_]/)[0]
  if (base === 'en' || base === 'uk') return base
  return null
}

function readStoredLocale(): Locale | null {
  try {
    return normalizeLocale(localStorage.getItem(STORAGE_KEY))
  } catch {
    return null
  }
}

function writeStoredLocale(locale: Locale) {
  try {
    localStorage.setItem(STORAGE_KEY, locale)
  } catch {
    /* ignore */
  }
}

type I18nContextValue = {
  locale: Locale
  setLocale: (locale: Locale) => Promise<void>
  t: (key: string, vars?: Record<string, string>) => string
}

const I18nContext = createContext<I18nContextValue | null>(null)

export function I18nProvider({ children }: { children: ReactNode }) {
  const { user, status, refreshStatus } = useAuth()
  const [locale, setLocaleState] = useState<Locale>(() => {
    return (
      normalizeLocale(user?.locale) ??
      readStoredLocale() ??
      normalizeLocale(status?.default_locale) ??
      'en'
    )
  })

  useEffect(() => {
    const next =
      normalizeLocale(user?.locale) ??
      readStoredLocale() ??
      normalizeLocale(status?.default_locale) ??
      'en'
    setLocaleState(next)
  }, [user?.locale, status?.default_locale])

  const setLocale = useCallback(
    async (next: Locale) => {
      setLocaleState(next)
      writeStoredLocale(next)
      if (user) {
        try {
          await api.updateMe({ locale: next })
          await refreshStatus()
        } catch {
          /* keep local preference even if PATCH fails */
        }
      }
    },
    [user, refreshStatus],
  )

  const t = useCallback(
    (key: string, vars?: Record<string, string>) => {
      const dict = dictionaries[locale] ?? dictionaries.en
      let text = dict[key] ?? dictionaries.en[key] ?? key
      if (vars) {
        for (const [k, v] of Object.entries(vars)) {
          text = text.replace(`{${k}}`, v)
        }
      }
      return text
    },
    [locale],
  )

  const value = useMemo(
    () => ({ locale, setLocale, t }),
    [locale, setLocale, t],
  )

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
}

export function useI18n(): I18nContextValue {
  const ctx = useContext(I18nContext)
  if (!ctx) {
    throw new Error('useI18n must be used within I18nProvider')
  }
  return ctx
}
