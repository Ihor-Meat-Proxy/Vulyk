export type TocHeading = {
  id: string
  text: string
  level: 2 | 3
}

/** GitHub-like slug for markdown headings (ASCII-focused). */
export function slugifyHeading(text: string): string {
  return text
    .trim()
    .toLowerCase()
    .replace(/[^\w\s-]/g, '')
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '')
}

/** Extract ## / ### headings for the in-page TOC. */
export function extractToc(markdown: string): TocHeading[] {
  const out: TocHeading[] = []
  const seen = new Map<string, number>()
  for (const line of markdown.split('\n')) {
    const m = /^(#{2,3})\s+(.+?)\s*$/.exec(line)
    if (!m) continue
    const level = m[1]!.length as 2 | 3
    const text = m[2]!.replace(/#+\s*$/, '').trim()
    if (!text) continue
    let id = slugifyHeading(text)
    const n = (seen.get(id) ?? 0) + 1
    seen.set(id, n)
    if (n > 1) id = `${id}-${n}`
    out.push({ id, text, level })
  }
  return out
}
