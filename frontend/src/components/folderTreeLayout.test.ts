import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

const css = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../index.css'),
  'utf8',
)

function ruleBody(selector: string): string {
  const re = new RegExp(
    `${selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\s*\\{([^}]+)\\}`,
  )
  const m = css.match(re)
  expect(m, `missing CSS rule for ${selector}`).toBeTruthy()
  return m![1]
}

describe('folder-tree count layout', () => {
  it('lays out the All-cases root as a flex row (not block)', () => {
    const body = ruleBody('.folder-tree-root')
    expect(body).toMatch(/display:\s*flex/)
    expect(body).not.toMatch(/display:\s*block/)
    expect(body).toMatch(/gap:/)
  })

  it('keeps folder counts muted, shrink-proof, and right-aligned', () => {
    const body = ruleBody('.folder-tree-count')
    expect(body).toMatch(/margin-left:\s*auto/)
    expect(body).toMatch(/flex-shrink:\s*0/)
    expect(body).toMatch(/color:\s*var\(--ink-muted\)/)
  })

  it('spaces coverage requirement titles away from their counts', () => {
    const body = ruleBody('.coverage-req-list > li')
    expect(body).toMatch(/display:\s*flex/)
    expect(body).toMatch(/gap:/)
  })
})
