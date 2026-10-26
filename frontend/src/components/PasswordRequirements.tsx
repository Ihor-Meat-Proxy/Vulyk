import { useI18n } from '../i18n'
import {
  passwordRequirementFlags,
  type PasswordPolicy,
} from '../passwordPolicy'
import { TestIds, testIdProps } from '../testing'

export function PasswordRequirements({
  password,
  policy,
}: {
  password: string
  policy: PasswordPolicy
}) {
  const { t } = useI18n()
  const flags = passwordRequirementFlags(password, policy)
  return (
    <ul
      className="muted"
      style={{ fontSize: '0.85rem', margin: '0.25rem 0 0', paddingLeft: '1.1rem' }}
      aria-live="polite"
      {...testIdProps(TestIds.passwordRequirements)}
    >
      <li data-ok={flags.length ? '1' : '0'}>
        {t('password.reqLength', { min: policy.min_length })}
        {password ? (flags.length ? ' ✓' : '') : ''}
      </li>
      {policy.require_letter && (
        <li data-ok={flags.letter ? '1' : '0'}>
          {t('password.reqLetter')}
          {password ? (flags.letter ? ' ✓' : '') : ''}
        </li>
      )}
      {policy.require_digit && (
        <li data-ok={flags.digit ? '1' : '0'}>
          {t('password.reqDigit')}
          {password ? (flags.digit ? ' ✓' : '') : ''}
        </li>
      )}
    </ul>
  )
}
