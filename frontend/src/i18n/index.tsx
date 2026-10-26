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
  'nav.suites': 'Folders',
  'nav.folders': 'Folders',
  'nav.allCases': 'All cases',
  'nav.close': 'Close',
  'nav.project': 'Project',
  'nav.signOut': 'Sign out',
  'nav.openMode': 'Open mode',
  'nav.locale': 'Language',
  'nav.appearance': 'Appearance',
  'status.untested': 'Untested',
  'status.passed': 'Passed',
  'status.failed': 'Failed',
  'status.blocked': 'Blocked',
  'status.skipped': 'Skipped',
  'status.retest': 'Retest',
  'folders.panel': 'Case folders',
  'folders.title': 'Folders',
  'folders.new': 'New folder',
  'folders.empty': 'No folders yet',
  'folders.promptName': 'Folder name',
  'folders.promptRename': 'Rename folder',
  'folders.deleteConfirm':
    'Folder is not empty. Type move_to_parent (cases go to parent) or delete_cases (retire keys).',
  'folders.deleteEmpty': 'Delete this folder?',
  'folders.deleteCasesForbidden': 'You do not have permission to delete cases.',
  'folders.addChild': 'Add subfolder',
  'folders.rename': 'Rename folder',
  'folders.delete': 'Delete folder',
  'folders.expand': 'Expand',
  'folders.collapse': 'Collapse',
  'folders.moveTo': 'Move to',
  'folders.moveCase': 'Move case to folder',
  'folders.root': 'Unfiled (root)',
  'cases.newTitle': 'New case title',
  'cases.create': 'Create case',
  'cases.creating': 'Creating…',
  'cases.search': 'Search cases (FTS)…',
  'cases.tag': 'Tag',
  'cases.statusFilter': 'Status filter',
  'cases.allStatuses': 'All statuses',
  'cases.count': 'cases',
  'cases.selected': 'selected',
  'cases.markReady': 'Mark ready',
  'cases.deprecate': 'Deprecate',
  'cases.generateAutomation': 'Generate automation',
  'cases.clearSelection': 'Clear',
  'cases.empty': 'No test cases in this view yet.',
  'generate.title': 'Generate',
  'generate.subtitle':
    'Turn requirements into case drafts, or cases into Playwright/pytest stubs that reuse your automation repo.',
  'generate.selectProject': 'Select a project to generate cases or code.',
  'generate.noPermission': 'You do not have permission to use LLM assist.',
  'generate.casesFromRequirements': 'Cases from requirements',
  'generate.automationCode': 'Automation code',
  'generate.model': 'Model',
  'generate.modelAutomation': 'Automation model',
  'generate.mockFallback': 'Mock (no providers configured)',
  'generate.requirements': 'Requirements',
  'generate.requirementsPlaceholder': 'Describe the behaviour to cover…',
  'generate.generateDrafts': 'Generate & save drafts',
  'generate.pickCases': 'Test cases',
  'generate.framework': 'Framework',
  'generate.generateCode': 'Generate code',
  'generate.needCases': 'Select at least one test case',
  'generate.casesNotice':
    'Generated {count} draft(s) via {provider}/{model}; persisted {persisted}.',
  'generate.codeNotice': 'Code from {provider}/{model}',
  'generate.contextFiles': 'Repository context used',
  'generate.download': 'Download code',
  'generate.searchCases': 'Search',
  'generate.searchCasesPlaceholder': 'Search by key or title…',
  'generate.folders': 'Folders',
  'generate.allFolders': 'All folders',
  'generate.cases': 'Cases',
  'generate.selectFolder': 'Select folder',
  'generate.noCasesSelected': 'No cases selected',
  'admin.automationRepo': 'Automation repo',
  'admin.automationRepoHelp':
    'Connect a read-only Git repo of existing tests. Vulyk clones it shallowly and indexes files for generation context.',
  'admin.automationRepoNeedProject': 'Select a project to configure its automation repo.',
  'admin.automationProvider': 'Provider',
  'admin.automationUrl': 'Remote URL',
  'admin.automationBranch': 'Branch',
  'admin.automationSubpath': 'Subpath (optional)',
  'admin.automationToken': 'Access token',
  'admin.automationTokenHint': 'Leave blank to keep existing',
  'admin.automationSave': 'Save connection',
  'admin.automationTest': 'Test connection',
  'admin.automationReindex': 'Re-index',
  'cases.colKey': 'Key',
  'cases.colTitle': 'Title',
  'cases.colPriority': 'Priority',
  'cases.colStatus': 'Status',
  'cases.loadMore': 'Load more',
  'setup.seedSample':
    'Seed sample projects “Kyiv Open-Air Concert 2026” and “Vulyk QA” (folders, cases, runs)',
  'appearance.title': 'Appearance',
  'appearance.subtitle':
    'Choose a theme, text size, density, and motion preferences. Saved to your account and applied instantly.',
  'appearance.theme': 'Theme',
  'appearance.theme.system': 'System (default)',
  'appearance.theme.light': 'Light',
  'appearance.theme.dark': 'Dark',
  'appearance.theme.high_contrast': 'High contrast',
  'appearance.theme.sepia': 'Sepia / night warm',
  'appearance.fontSize': 'Font size',
  'appearance.density': 'Density',
  'appearance.density.comfortable': 'Comfortable',
  'appearance.density.compact': 'Compact',
  'appearance.reducedMotion': 'Reduced motion',
  'appearance.reducedMotion.system': 'Follow system',
  'appearance.reducedMotion.on': 'Always reduce',
  'appearance.reducedMotion.off': 'Allow motion',
  'appearance.reducedMotion.hint':
    'Also respects the prefers-reduced-motion setting on your device.',
  'appearance.statusPreview': 'Status colours (icon + label)',
  'admin.defaultTheme': 'Default theme',
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
  'nav.suites': 'Теки',
  'nav.folders': 'Теки',
  'nav.allCases': 'Усі кейси',
  'nav.close': 'Закрити',
  'nav.project': 'Проєкт',
  'nav.signOut': 'Вийти',
  'nav.openMode': 'Відкритий режим',
  'nav.locale': 'Мова',
  'nav.appearance': 'Вигляд',
  'status.untested': 'Не пройдено',
  'status.passed': 'Пройдено',
  'status.failed': 'Провалено',
  'status.blocked': 'Заблоковано',
  'status.skipped': 'Пропущено',
  'status.retest': 'Повтор',
  'folders.panel': 'Теки кейсів',
  'folders.title': 'Теки',
  'folders.new': 'Нова тека',
  'folders.empty': 'Тек ще немає',
  'folders.promptName': 'Назва теки',
  'folders.promptRename': 'Перейменувати теку',
  'folders.deleteConfirm':
    'Тека не порожня. Введіть move_to_parent (кейси до батьківської) або delete_cases (ключі виводяться з обігу).',
  'folders.deleteEmpty': 'Видалити цю теку?',
  'folders.deleteCasesForbidden': 'Немає дозволу видаляти кейси.',
  'folders.addChild': 'Додати підтеку',
  'folders.rename': 'Перейменувати',
  'folders.delete': 'Видалити теку',
  'folders.expand': 'Розгорнути',
  'folders.collapse': 'Згорнути',
  'folders.moveTo': 'Перемістити',
  'folders.moveCase': 'Перемістити кейс до теки',
  'folders.root': 'Без теки (корінь)',
  'cases.newTitle': 'Назва нового кейсу',
  'cases.create': 'Створити кейс',
  'cases.creating': 'Створення…',
  'cases.search': 'Пошук кейсів…',
  'cases.tag': 'Тег',
  'cases.statusFilter': 'Фільтр статусу',
  'cases.allStatuses': 'Усі статуси',
  'cases.count': 'кейсів',
  'cases.selected': 'обрано',
  'cases.markReady': 'Позначити готовим',
  'cases.deprecate': 'Застаріти',
  'cases.generateAutomation': 'Згенерувати автоматизацію',
  'cases.clearSelection': 'Скинути',
  'cases.empty': 'У цьому перегляді ще немає кейсів.',
  'cases.colKey': 'Ключ',
  'cases.colTitle': 'Назва',
  'cases.colPriority': 'Пріоритет',
  'cases.colStatus': 'Статус',
  'cases.loadMore': 'Ще',
  'generate.title': 'Генерація',
  'generate.subtitle':
    'Перетворіть вимоги на чернетки кейсів або кейси на заготовки Playwright/pytest з урахуванням репозиторію автоматизації.',
  'generate.selectProject': 'Оберіть проєкт для генерації кейсів або коду.',
  'generate.noPermission': 'Немає дозволу на LLM-асистент.',
  'generate.casesFromRequirements': 'Кейси з вимог',
  'generate.automationCode': 'Код автоматизації',
  'generate.model': 'Модель',
  'generate.modelAutomation': 'Модель для автоматизації',
  'generate.mockFallback': 'Mock (провайдери не налаштовані)',
  'generate.requirements': 'Вимоги',
  'generate.requirementsPlaceholder': 'Опишіть поведінку для покриття…',
  'generate.generateDrafts': 'Згенерувати і зберегти чернетки',
  'generate.pickCases': 'Тест-кейси',
  'generate.framework': 'Фреймворк',
  'generate.generateCode': 'Згенерувати код',
  'generate.needCases': 'Оберіть хоча б один тест-кейс',
  'generate.casesNotice':
    'Згенеровано {count} чернеток через {provider}/{model}; збережено {persisted}.',
  'generate.codeNotice': 'Код від {provider}/{model}',
  'generate.contextFiles': 'Використаний контекст репозиторію',
  'generate.download': 'Завантажити код',
  'generate.searchCases': 'Пошук',
  'generate.searchCasesPlaceholder': 'Пошук за ключем або назвою…',
  'generate.folders': 'Теки',
  'generate.allFolders': 'Усі теки',
  'generate.cases': 'Кейси',
  'generate.selectFolder': 'Обрати теку',
  'generate.noCasesSelected': 'Кейси не обрані',
  'admin.automationRepo': 'Репозиторій автоматизації',
  'admin.automationRepoHelp':
    'Підключіть Git-репозиторій існуючих тестів (лише читання). Vulyk зробить shallow clone і проіндексує файли для контексту генерації.',
  'admin.automationRepoNeedProject':
    'Оберіть проєкт, щоб налаштувати репозиторій автоматизації.',
  'admin.automationProvider': 'Провайдер',
  'admin.automationUrl': 'URL репозиторію',
  'admin.automationBranch': 'Гілка',
  'admin.automationSubpath': 'Підшлях (необовʼязково)',
  'admin.automationToken': 'Токен доступу',
  'admin.automationTokenHint': 'Залиште порожнім, щоб зберегти наявний',
  'admin.automationSave': 'Зберегти підключення',
  'admin.automationTest': 'Перевірити зʼєднання',
  'admin.automationReindex': 'Переіндексувати',
  'setup.seedSample':
    'Створити зразки «Київський open-air концерт 2026» і «Vulyk QA» (теки, кейси, прогони)',
  'appearance.title': 'Вигляд',
  'appearance.subtitle':
    'Оберіть тему, розмір тексту, щільність і рух. Зберігається в обліковому записі й застосовується миттєво.',
  'appearance.theme': 'Тема',
  'appearance.theme.system': 'Системна (за замовчуванням)',
  'appearance.theme.light': 'Світла',
  'appearance.theme.dark': 'Темна',
  'appearance.theme.high_contrast': 'Високий контраст',
  'appearance.theme.sepia': 'Сепія / теплий нічний',
  'appearance.fontSize': 'Розмір шрифту',
  'appearance.density': 'Щільність',
  'appearance.density.comfortable': 'Зручна',
  'appearance.density.compact': 'Компактна',
  'appearance.reducedMotion': 'Зменшений рух',
  'appearance.reducedMotion.system': 'За системою',
  'appearance.reducedMotion.on': 'Завжди зменшувати',
  'appearance.reducedMotion.off': 'Дозволити рух',
  'appearance.reducedMotion.hint':
    'Також враховує prefers-reduced-motion на вашому пристрої.',
  'appearance.statusPreview': 'Кольори статусів (іконка + підпис)',
  'admin.defaultTheme': 'Тема за замовчуванням',
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
