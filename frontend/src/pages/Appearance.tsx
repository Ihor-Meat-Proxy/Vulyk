import { StatusBadge } from '../components/StatusBadge'
import { useI18n } from '../i18n'
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
    <div className="appearance-page">
      <header>
        <h2>{t('appearance.title')}</h2>
        <p className="muted">{t('appearance.subtitle')}</p>
      </header>

      <section className="panel" aria-labelledby="appearance-theme">
        <h3 id="appearance-theme">{t('appearance.theme')}</h3>
        <label className="field">
          <span className="sr-only">{t('appearance.theme')}</span>
          <select
            value={prefs.theme}
            aria-label={t('appearance.theme')}
            onChange={(e) => void setTheme(e.target.value as ThemeChoice)}
          >
            {THEMES.map((theme) => (
              <option key={theme} value={theme}>
                {t(`appearance.theme.${theme}`)}
              </option>
            ))}
          </select>
        </label>
      </section>

      <section className="panel" aria-labelledby="appearance-font">
        <h3 id="appearance-font">{t('appearance.fontSize')}</h3>
        <div className="font-size-steps" role="group" aria-label={t('appearance.fontSize')}>
          {FONT_SIZE_STEPS.map((step) => (
            <button
              key={step}
              type="button"
              className="btn secondary"
              aria-pressed={prefs.fontSize === step}
              onClick={() => void setFontSize(step)}
            >
              {step}%
            </button>
          ))}
        </div>
      </section>

      <section className="panel" aria-labelledby="appearance-density">
        <h3 id="appearance-density">{t('appearance.density')}</h3>
        <label className="field">
          <span className="sr-only">{t('appearance.density')}</span>
          <select
            value={prefs.density}
            aria-label={t('appearance.density')}
            onChange={(e) => void setDensity(e.target.value as Density)}
          >
            {DENSITIES.map((d) => (
              <option key={d} value={d}>
                {t(`appearance.density.${d}`)}
              </option>
            ))}
          </select>
        </label>
      </section>

      <section className="panel" aria-labelledby="appearance-motion">
        <h3 id="appearance-motion">{t('appearance.reducedMotion')}</h3>
        <label className="field">
          <span className="sr-only">{t('appearance.reducedMotion')}</span>
          <select
            value={
              prefs.reducedMotion === null
                ? 'system'
                : prefs.reducedMotion
                  ? 'on'
                  : 'off'
            }
            aria-label={t('appearance.reducedMotion')}
            onChange={(e) => {
              const v = e.target.value
              void setReducedMotion(
                v === 'system' ? null : v === 'on' ? true : false,
              )
            }}
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

      <section className="panel" aria-labelledby="appearance-preview">
        <h3 id="appearance-preview">{t('appearance.statusPreview')}</h3>
        <div className="appearance-preview">
          {(
            [
              'Passed',
              'Failed',
              'Blocked',
              'Skipped',
              'Retest',
              'Untested',
            ] as const
          ).map((s) => (
            <StatusBadge key={s} status={s} />
          ))}
        </div>
      </section>
    </div>
  )
}
