/** IndexedDB helpers for offline read cache + step-result queue. */

const DB_NAME = 'vulyk-offline'
const DB_VERSION = 1

export type CachedCase = {
  key: string
  projectId: string
  caseId: string
  viewedAt: number
  payload: unknown
}

export type CachedRun = {
  key: string
  projectId: string
  runId: string
  viewedAt: number
  payload: unknown
}

export type QueuedStepResult = {
  id: string
  projectId: string
  runId: string
  body: Record<string, unknown>
  createdAt: number
  attempts: number
  lastError?: string
}

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION)
    req.onupgradeneeded = () => {
      const db = req.result
      if (!db.objectStoreNames.contains('cases')) {
        const cases = db.createObjectStore('cases', { keyPath: 'key' })
        cases.createIndex('viewedAt', 'viewedAt')
      }
      if (!db.objectStoreNames.contains('runs')) {
        const runs = db.createObjectStore('runs', { keyPath: 'key' })
        runs.createIndex('viewedAt', 'viewedAt')
      }
      if (!db.objectStoreNames.contains('queue')) {
        db.createObjectStore('queue', { keyPath: 'id' })
      }
    }
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error ?? new Error('indexedDB open failed'))
  })
}

function txDone(tx: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve()
    tx.onerror = () => reject(tx.error ?? new Error('indexedDB tx failed'))
    tx.onabort = () => reject(tx.error ?? new Error('indexedDB tx aborted'))
  })
}

const MAX_CASES = 40
const MAX_RUNS = 20

async function pruneStore(
  storeName: 'cases' | 'runs',
  max: number,
): Promise<void> {
  const db = await openDb()
  const tx = db.transaction(storeName, 'readwrite')
  const store = tx.objectStore(storeName)
  const index = store.index('viewedAt')
  const all: { key: string; viewedAt: number }[] = await new Promise(
    (resolve, reject) => {
      const req = index.getAll()
      req.onsuccess = () => resolve(req.result as { key: string; viewedAt: number }[])
      req.onerror = () => reject(req.error)
    },
  )
  if (all.length <= max) {
    await txDone(tx)
    db.close()
    return
  }
  all
    .sort((a, b) => a.viewedAt - b.viewedAt)
    .slice(0, all.length - max)
    .forEach((row) => store.delete(row.key))
  await txDone(tx)
  db.close()
}

export async function cacheCase(
  projectId: string,
  caseId: string,
  payload: unknown,
): Promise<void> {
  if (typeof indexedDB === 'undefined') return
  const db = await openDb()
  const tx = db.transaction('cases', 'readwrite')
  tx.objectStore('cases').put({
    key: `${projectId}:${caseId}`,
    projectId,
    caseId,
    viewedAt: Date.now(),
    payload,
  } satisfies CachedCase)
  await txDone(tx)
  db.close()
  await pruneStore('cases', MAX_CASES)
}

export async function getCachedCase(
  projectId: string,
  caseId: string,
): Promise<unknown | null> {
  if (typeof indexedDB === 'undefined') return null
  const db = await openDb()
  const tx = db.transaction('cases', 'readonly')
  const row = await new Promise<CachedCase | undefined>((resolve, reject) => {
    const req = tx.objectStore('cases').get(`${projectId}:${caseId}`)
    req.onsuccess = () => resolve(req.result as CachedCase | undefined)
    req.onerror = () => reject(req.error)
  })
  await txDone(tx)
  db.close()
  return row?.payload ?? null
}

export async function cacheRun(
  projectId: string,
  runId: string,
  payload: unknown,
): Promise<void> {
  if (typeof indexedDB === 'undefined') return
  const db = await openDb()
  const tx = db.transaction('runs', 'readwrite')
  tx.objectStore('runs').put({
    key: `${projectId}:${runId}`,
    projectId,
    runId,
    viewedAt: Date.now(),
    payload,
  } satisfies CachedRun)
  await txDone(tx)
  db.close()
  await pruneStore('runs', MAX_RUNS)
}

export async function getCachedRun(
  projectId: string,
  runId: string,
): Promise<unknown | null> {
  if (typeof indexedDB === 'undefined') return null
  const db = await openDb()
  const tx = db.transaction('runs', 'readonly')
  const row = await new Promise<CachedRun | undefined>((resolve, reject) => {
    const req = tx.objectStore('runs').get(`${projectId}:${runId}`)
    req.onsuccess = () => resolve(req.result as CachedRun | undefined)
    req.onerror = () => reject(req.error)
  })
  await txDone(tx)
  db.close()
  return row?.payload ?? null
}

export async function listCachedCases(): Promise<CachedCase[]> {
  if (typeof indexedDB === 'undefined') return []
  const db = await openDb()
  const tx = db.transaction('cases', 'readonly')
  const rows = await new Promise<CachedCase[]>((resolve, reject) => {
    const req = tx.objectStore('cases').getAll()
    req.onsuccess = () => resolve(req.result as CachedCase[])
    req.onerror = () => reject(req.error)
  })
  await txDone(tx)
  db.close()
  return rows.sort((a, b) => b.viewedAt - a.viewedAt)
}

export async function listCachedRuns(): Promise<CachedRun[]> {
  if (typeof indexedDB === 'undefined') return []
  const db = await openDb()
  const tx = db.transaction('runs', 'readonly')
  const rows = await new Promise<CachedRun[]>((resolve, reject) => {
    const req = tx.objectStore('runs').getAll()
    req.onsuccess = () => resolve(req.result as CachedRun[])
    req.onerror = () => reject(req.error)
  })
  await txDone(tx)
  db.close()
  return rows.sort((a, b) => b.viewedAt - a.viewedAt)
}

export async function enqueueStepResult(
  item: Omit<QueuedStepResult, 'id' | 'createdAt' | 'attempts'> & {
    id?: string
  },
): Promise<QueuedStepResult> {
  const entry: QueuedStepResult = {
    id: item.id ?? crypto.randomUUID(),
    projectId: item.projectId,
    runId: item.runId,
    body: item.body,
    createdAt: Date.now(),
    attempts: 0,
  }
  const db = await openDb()
  const tx = db.transaction('queue', 'readwrite')
  tx.objectStore('queue').put(entry)
  await txDone(tx)
  db.close()
  return entry
}

export async function listQueuedStepResults(): Promise<QueuedStepResult[]> {
  if (typeof indexedDB === 'undefined') return []
  const db = await openDb()
  const tx = db.transaction('queue', 'readonly')
  const rows = await new Promise<QueuedStepResult[]>((resolve, reject) => {
    const req = tx.objectStore('queue').getAll()
    req.onsuccess = () => resolve(req.result as QueuedStepResult[])
    req.onerror = () => reject(req.error)
  })
  await txDone(tx)
  db.close()
  return rows.sort((a, b) => a.createdAt - b.createdAt)
}

export async function removeQueuedStepResult(id: string): Promise<void> {
  const db = await openDb()
  const tx = db.transaction('queue', 'readwrite')
  tx.objectStore('queue').delete(id)
  await txDone(tx)
  db.close()
}

export async function updateQueuedStepResult(
  entry: QueuedStepResult,
): Promise<void> {
  const db = await openDb()
  const tx = db.transaction('queue', 'readwrite')
  tx.objectStore('queue').put(entry)
  await txDone(tx)
  db.close()
}
