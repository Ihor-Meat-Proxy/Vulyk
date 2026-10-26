import { StatusBadge } from '../components/StatusBadge'
import { useI18n } from '../i18n'
import { TestIds, listItemProps, testIdProps } from '../testing'
import { useAppearance } from '../theme/appearance'
import { FONT_SIZE_STEPS, type Density, type ThemeChoice } from '../theme/tokens'

const THEMES: ThemeChoice[] = [
  'system',
  'light',
  'dark',
  'high_contrast',
  'sepia',
]

const DENSITIES: Density[] = ['comfortable', 'compact']

export function Appearance() {
  const { t } = useI18n()
  const {
    prefs,
    setTheme,
    setFontSize,
    setDensity,
    setReducedMotion,
  } = useAppearance()

  return (
    <div className="appearance-page" {...testIdProps(TestIds.appearancePage)}>
      <header>
        <h2>{t('appearance.title')}</h2>
        <p className="muted">{t('appearance.subtitle')}</p>
      </header>

      <section className="panel">
        <label className="field">
          <span>{t('appearance.theme')}</span>
          <select
            value={prefs.theme}
            onChange={(e) => void setTheme(e.target.value as ThemeChoice)}
            {...testIdProps(TestIds.appearanceThemeSelect)}
          >
            {THEMES.map((theme) => (
              <option key={theme} value={theme}>
                {t(`appearance.theme.${theme}`)}
              </option>
            ))}
          </select>
        </label>
      </section>

      <section className="panel">
        <h3>{t('appearance.fontSize')}</h3>
        <div
          className="font-size-steps"
          role="group"
          aria-label={t('appearance.fontSize')}
          {...testIdProps(TestIds.appearanceFontSizeSelect)}
        >
          {FONT_SIZE_STEPS.map((step, index) => (
            <button
              key={step}
              type="button"
              className="btn secondary"
              aria-pressed={prefs.fontSize === step}
              onClick={() => void setFontSize(step)}
              {...listItemProps(TestIds.appearanceFontSizeItem, {
                entityId: String(step),
                entityKey: String(step),
                index,
              })}
            >
              {step}%
            </button>
          ))}
        </div>
      </section>

      <section className="panel">
        <label className="field">
          <span>{t('appearance.density')}</span>
          <select
            value={prefs.density}
            onChange={(e) => void setDensity(e.target.value as Density)}
            {...testIdProps(TestIds.appearanceDensitySelect)}
          >
            {DENSITIES.map((d) => (
              <option key={d} value={d}>
                {t(`appearance.density.${d}`)}
              </option>
            ))}
          </select>
        </label>
      </section>

      <section className="panel">
        <label className="field">
          <span>{t('appearance.reducedMotion')}</span>
          <select
            value={
              prefs.reducedMotion === null
                ? 'system'
                : prefs.reducedMotion
                  ? 'on'
                  : 'off'
            }
            onChange={(e) => {
              const v = e.target.value
              void setReducedMotion(
                v === 'system' ? null : v === 'on' ? true : false,
              )
            }}
            {...testIdProps(TestIds.appearanceReducedMotionCheckbox)}
          >
            <option value="system">{t('appearance.reducedMotion.system')}</option>
            <option value="on">{t('appearance.reducedMotion.on')}</option>
            <option value="off">{t('appearance.reducedMotion.off')}</option>
          </select>
        </label>
        <p className="muted" style={{ marginTop: '0.5rem', fontSize: '0.85rem' }}>
          {t('appearance.reducedMotion.hint')}
        </p>
      </section>

      <section className="panel">
        <h3>{t('appearance.statusPreview')}</h3>
        <div className="appearance-preview">
          {(
            [
              'Passed',
              'Failed',
              'Blocked',
              'Skipped',
              'Retest',
              'Untested',
              'Ready',
              'Running',
              'Untestable',
              'Completed',
              'In_progress',
              'Cancelled',
              'Open',
              'Closed',
            ] as const
          ).map((s) => (
            <StatusBadge key={s} status={s} testId="appearance-status-preview" />
          ))}
        </div>
      </section>
    </div>
  )
}
