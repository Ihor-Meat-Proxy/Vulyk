import { describe, expect, it } from 'vitest'
import en from '../../i18n/locales/en'
import uk from '../../i18n/locales/uk'
import fr from '../../i18n/locales/fr'
import es from '../../i18n/locales/es'
import de from '../../i18n/locales/de'

const required = [
  'admin.backups',
  'admin.backupsHint',
  'admin.backupsRunNow',
  'admin.backupsHistory',
  'admin.backupsTestConnection',
]

describe('backups i18n', () => {
  for (const [name, dict] of [
    ['en', en],
    ['uk', uk],
    ['fr', fr],
    ['es', es],
    ['de', de],
  ] as const) {
    it(`${name} has backup keys`, () => {
      for (const key of required) {
        expect(dict[key], key).toBeTruthy()
      }
    })
  }
})
