/** UUID v1–v5 path segment (case-insensitive). */
const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

export function isEntityUuid(value: string | undefined | null): boolean {
  return Boolean(value && UUID_RE.test(value.trim()))
}

/** Prefer slug, then display_id, then UUID for URL segments. */
export function runRef(run: {
  id: string
  slug?: string | null
  display_id?: string | null
}): string {
  const slug = run.slug?.trim()
  if (slug) return slug
  const display = run.display_id?.trim()
  if (display) return display
  return run.id
}

export function iterationRef(iteration: {
  id: string
  display_id?: string | null
}): string {
  const display = iteration.display_id?.trim()
  if (display) return display
  return iteration.id
}

export function campaignRef(campaign: {
  id: string
  display_id?: string | null
}): string {
  const display = campaign.display_id?.trim()
  if (display) return display
  return campaign.id
}

export function runPath(run: {
  id: string
  slug?: string | null
  display_id?: string | null
}): string {
  return `/runs/${encodeURIComponent(runRef(run))}`
}

export function executionRunPath(
  run: { id: string; slug?: string | null; display_id?: string | null },
  caseId?: string,
): string {
  const base = `/executions/runs/${encodeURIComponent(runRef(run))}`
  return caseId ? `${base}/cases/${encodeURIComponent(caseId)}` : base
}

export function iterationPath(iteration: {
  id: string
  display_id?: string | null
}): string {
  return `/executions/iterations/${encodeURIComponent(iterationRef(iteration))}`
}

export function reportRunPath(run: {
  id: string
  slug?: string | null
  display_id?: string | null
}): string {
  return `/reports/runs/${encodeURIComponent(runRef(run))}`
}
