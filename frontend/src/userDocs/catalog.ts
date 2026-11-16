import manifestJson from '../../../docs/user/_manifest.json'
import overviewMd from '../../../docs/user/overview.md?raw'
import conceptsMd from '../../../docs/user/concepts.md?raw'
import casesMd from '../../../docs/user/cases.md?raw'
import requirementsMd from '../../../docs/user/requirements.md?raw'
import importMd from '../../../docs/user/import.md?raw'
import aiMd from '../../../docs/user/ai.md?raw'
import runsMd from '../../../docs/user/runs.md?raw'
import cicdMd from '../../../docs/user/cicd.md?raw'
import adminMd from '../../../docs/user/admin.md?raw'
import apiMd from '../../../docs/user/api.md?raw'
import troubleshootingMd from '../../../docs/user/troubleshooting.md?raw'

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
  'concepts.md': conceptsMd,
  'cases.md': casesMd,
  'requirements.md': requirementsMd,
  'import.md': importMd,
  'ai.md': aiMd,
  'runs.md': runsMd,
  'cicd.md': cicdMd,
  'admin.md': adminMd,
  'api.md': apiMd,
  'troubleshooting.md': troubleshootingMd,
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
