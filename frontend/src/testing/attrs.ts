import { assertSafeTestAttrs, isSafeEntityKey } from './security'
import { childTestId } from './testIds'

/** When true, omit data-testid / list data-* attrs from the DOM (default: keep them). */
export function stripTestIdsEnabled(): boolean {
  return import.meta.env.VITE_STRIP_TEST_IDS === 'true'
}

export type TestIdProps = { 'data-testid'?: string }

/** Spread onto an element: `{...testIdProps(TestIds.loginEmailInput)}` */
export function testIdProps(id: string): TestIdProps {
  if (stripTestIdsEnabled()) return {}
  const props = { 'data-testid': id }
  assertSafeTestAttrs(props)
  return props
}

export type ListItemOptions = {
  entityId: string
  entityKey?: string | null
  index: number
  depth?: number
}

export type ListItemProps = {
  'data-testid'?: string
  'data-entity-id'?: string
  'data-entity-key'?: string
  'data-index'?: string
  'data-depth'?: string
}

/**
 * List/table/tree row attributes.
 * Example: `{...listItemProps(TestIds.caseRow, { entityId, entityKey: c.key, index })}`
 *
 * Never pass emails, display names, passwords, or token secrets as entityId/entityKey.
 * Freeform display names (spaces, etc.) are omitted — only short API/URL keys are emitted —
 * so a bad call site cannot crash the page via assertSafeTestAttrs.
 */
export function listItemProps(
  testId: string,
  opts: ListItemOptions,
): ListItemProps {
  if (stripTestIdsEnabled()) return {}
  const props: ListItemProps = {
    'data-testid': testId,
    'data-entity-id': opts.entityId,
    'data-index': String(opts.index),
  }
  if (isSafeEntityKey(opts.entityKey)) {
    props['data-entity-key'] = opts.entityKey
  }
  if (opts.depth != null) {
    props['data-depth'] = String(opts.depth)
  }
  assertSafeTestAttrs(props)
  return props
}

/** Scoped child under a list item base id. */
export function listChildProps(base: string, child: string): TestIdProps {
  return testIdProps(childTestId(base, child))
}
