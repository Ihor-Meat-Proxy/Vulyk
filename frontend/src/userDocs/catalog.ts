import manifestJson from '../../../docs/user/_manifest.json'
import overviewMd from '../../../docs/user/overview.md?raw'

export type UserDocPage = {
  id: string
  file: string
  title: string
  markdown: string
}

type ManifestEntry = {
  id: string
  file: string
  title: string
}

const manifest = manifestJson as ManifestEntry[]

/** Raw markdown keyed by filename (eager so offline/PWA builds embed every page). */
const rawByFile: Record<string, string> = {
  'overview.md': overviewMd,
}

export function listUserDocPages(): UserDocPage[] {
  return manifest.map((entry) => {
    const markdown = rawByFile[entry.file]
    if (markdown == null) {
      throw new Error(`Missing bundled user doc: ${entry.file}`)
    }
    return {
      id: entry.id,
      file: entry.file,
      title: entry.title,
      markdown,
    }
  })
}

export function getUserDocPage(id: string): UserDocPage | undefined {
  return listUserDocPages().find((p) => p.id === id)
}
