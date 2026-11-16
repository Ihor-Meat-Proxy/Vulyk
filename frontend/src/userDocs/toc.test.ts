import { describe, expect, it } from 'vitest'
import { extractToc, slugifyHeading } from './toc'

describe('userDocs toc', () => {
  it('slugifyHeading strips punctuation', () => {
    expect(slugifyHeading('Hello, World!')).toBe('hello-world')
  })

  it('extractToc reads h2/h3 and uniquifies ids', () => {
    const md = `# Title\n\n## One\n\n### Nested\n\n## One\n`
    expect(extractToc(md)).toEqual([
      { id: 'one', text: 'One', level: 2 },
      { id: 'nested', text: 'Nested', level: 3 },
      { id: 'one-2', text: 'One', level: 2 },
    ])
  })
})
