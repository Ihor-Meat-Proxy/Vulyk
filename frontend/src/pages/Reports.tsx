import { useCallback, useEffect, useState } from 'react'
import {
  api,
  type FlakyReport,
  type PassFailTrends,
  type ProjectDashboard,
  type RequirementCoverageReport,
  type RunSummaryReport,
} from '../api'
import { useProjectPermissions } from '../hooks/useProjectPermissions'

type Props = {
  projectId: string | null
}

function pct(n: number) {
  return `${n.toFixed(1)}%`
}

function CountsBar({
  passed,
  failed,
  other,
}: {
  passed: number
  failed: number
  other: number
}) {
  const total = Math.max(passed + failed + other, 1)
  return (
    <div>
      <div
        className="trend-bar"
        role="img"
        aria-label={`Passed ${passed}, Failed ${failed}, Other ${other}`}
      >
        <span className="trend-seg passed" style={{ width: `${(passed / total) * 100}%` }} />
        <span className="trend-seg failed" style={{ width: `${(failed / total) * 100}%` }} />
        <span className="trend-seg other" style={{ width: `${(other / total) * 100}%` }} />
      </div>
      <div className="progress-legend">
        <span className="progress-legend-item">
          <span className="progress-legend-swatch passed" aria-hidden />
          Passed {passed}
        </span>
        <span className="progress-legend-item">
          <span className="progress-legend-swatch failed" aria-hidden />
          Failed {failed}
        </span>
        <span className="progress-legend-item">
          <span className="progress-legend-swatch skipped" aria-hidden />
          Other {other}
        </span>
      </div>
    </div>
  )
}

export function Reports({ projectId }: Props) {
  const { can } = useProjectPermissions(projectId)
  const canExport = can('export_reports')
  const [dash, setDash] = useState<ProjectDashboard | null>(null)
  const [trends, setTrends] = useState<PassFailTrends | null>(null)
  const [coverage, setCoverage] = useState<RequirementCoverageReport | null>(null)
  const [flaky, setFlaky] = useState<FlakyReport | null>(null)
  const [selectedSummary, setSelectedSummary] = useState<RunSummaryReport | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const refresh = useCallback(async () => {
    if (!projectId) {
      setDash(null)
      setTrends(null)
      setCoverage(null)
      setFlaky(null)
      return
    }
    setBusy(true)
    try {
      const [d, t, c, f] = await Promise.all([
        api.getDashboard(projectId),
        api.getTrends(projectId, 'run', 12),
        api.getCoverage(projectId),
        api.getFlaky(projectId),
      ])
      setDash(d)
      setTrends(t)
      setCoverage(c)
      setFlaky(f)
      setSelectedSummary(d.recent_run_summaries[0] ?? null)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }, [projectId])

  useEffect(() => {
    void refresh()
  }, [refresh])

  if (!projectId) {
    return <p className="muted">Select a project first.</p>
  }

  const onExport = async (
    format: 'csv' | 'pdf',
    report = 'dashboard',
    runId?: string,
  ) => {
    try {
      await api.downloadReportExport(projectId, { format, report, runId })
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  return (
    <div className="reports-page">
      <div className="panel-header">
        <div>
          <h2>Reports</h2>
          <p className="muted">
            Run summaries, pass/fail trends, requirement coverage, and flaky detection.
          </p>
        </div>
        <div className="topbar-actions">
          <button type="button" className="btn secondary" onClick={() => void refresh()} disabled={busy}>
            Refresh
          </button>
          {canExport && (
            <button type="button" className="btn secondary" onClick={() => void onExport('csv')}>
              Export CSV
            </button>
          )}
          {canExport && (
            <button type="button" className="btn" onClick={() => void onExport('pdf')}>
              Export PDF
            </button>
          )}
        </div>
      </div>

      {error && <p className="error">{error}</p>}

      {dash && (
        <section className="report-kpis" aria-label="Project summary">
          <div className="kpi">
            <span className="kpi-label">Cases</span>
            <span className="kpi-value">{dash.total_cases}</span>
          </div>
          <div className="kpi">
            <span className="kpi-label">Runs</span>
            <span className="kpi-value">{dash.total_runs}</span>
          </div>
          <div className="kpi">
            <span className="kpi-label">Requirements</span>
            <span className="kpi-value">{dash.total_requirements}</span>
          </div>
          <div className="kpi">
            <span className="kpi-label">Coverage</span>
            <span className="kpi-value">{pct(dash.coverage.coverage_percent)}</span>
          </div>
        </section>
      )}

      <div className="reports-grid">
        <section className="panel">
          <h3>Recent run summaries</h3>
          {!dash?.recent_run_summaries.length && (
            <p className="muted">No runs yet. Create a run and ingest results to see summaries.</p>
          )}
          <ul className="report-list">
            {dash?.recent_run_summaries.map((s) => (
              <li key={s.run_id}>
                <button
                  type="button"
                  className={selectedSummary?.run_id === s.run_id ? 'active' : undefined}
                  onClick={() => setSelectedSummary(s)}
                >
                  <strong>{s.name}</strong>
                  <span className="muted">
                    {pct(s.pass_rate)} · {s.counts.passed}P / {s.counts.failed}F
                  </span>
                  <CountsBar
                    passed={s.counts.passed}
                    failed={s.counts.failed + s.counts.error}
                    other={s.counts.skipped + s.counts.blocked + s.counts.untested}
                  />
                </button>
              </li>
            ))}
          </ul>
          {selectedSummary && (
            <div className="report-detail">
              <h4>{selectedSummary.name}</h4>
              <p className="muted">Status: {selectedSummary.status}</p>
              <dl className="counts-dl">
                <div>
                  <dt>Passed</dt>
                  <dd>{selectedSummary.counts.passed}</dd>
                </div>
                <div>
                  <dt>Failed</dt>
                  <dd>{selectedSummary.counts.failed}</dd>
                </div>
                <div>
                  <dt>Skipped</dt>
                  <dd>{selectedSummary.counts.skipped}</dd>
                </div>
                <div>
                  <dt>Pass rate</dt>
                  <dd>{pct(selectedSummary.pass_rate)}</dd>
                </div>
              </dl>
              <button
                type="button"
                className="btn secondary"
                onClick={() => void onExport('csv', 'run', selectedSummary.run_id)}
              >
                Export this run (CSV)
              </button>
            </div>
          )}
        </section>

        <section className="panel">
          <h3>Pass / fail trends</h3>
          {!trends?.points.length && <p className="muted">No trend data yet.</p>}
          <ul className="trend-list">
            {trends?.points.map((p, idx) => (
              <li key={`${p.bucket}-${p.run_id ?? idx}`}>
                <div className="trend-row">
                  <span>{p.run_name ?? p.bucket}</span>
                  <span className="muted">{pct(p.pass_rate)}</span>
                </div>
                <CountsBar
                  passed={p.counts.passed}
                  failed={p.counts.failed + p.counts.error}
                  other={p.counts.skipped + p.counts.blocked}
                />
              </li>
            ))}
          </ul>
          <button
            type="button"
            className="btn secondary"
            onClick={() => void onExport('csv', 'trends')}
          >
            Export trends CSV
          </button>
        </section>

        <section className="panel">
          <h3>Coverage by requirement</h3>
          {!coverage?.rows.length && (
            <p className="muted">No requirements linked yet.</p>
          )}
          <table className="data-table">
            <thead>
              <tr>
                <th>Requirement</th>
                <th>Linked</th>
                <th>Passed</th>
                <th>Failed</th>
                <th>Pass %</th>
              </tr>
            </thead>
            <tbody>
              {coverage?.rows.map((r) => (
                <tr key={r.requirement_id}>
                  <td>
                    {r.external_key ? `${r.external_key} · ` : ''}
                    {r.title}
                  </td>
                  <td>{r.linked_cases}</td>
                  <td>{r.cases_passed}</td>
                  <td>{r.cases_failed}</td>
                  <td>{pct(r.pass_percent)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <button
            type="button"
            className="btn secondary"
            onClick={() => void onExport('csv', 'coverage')}
          >
            Export coverage CSV
          </button>
        </section>

        <section className="panel">
          <h3>Flaky tests</h3>
          <p className="muted">
            Cases that both passed and failed across the last{' '}
            {flaky?.lookback_runs ?? '—'} runs.
          </p>
          {!flaky?.cases.length && <p className="muted">No flaky cases detected.</p>}
          <ul className="report-list">
            {flaky?.cases.map((c) => (
              <li key={c.case_id}>
                <strong>
                  {c.case_key} — {c.title}
                </strong>
                <span className="muted">
                  flake {c.flake_score.toFixed(2)} · {c.passes}P / {c.failures}F over{' '}
                  {c.runs_seen} runs · {c.last_statuses.join(' → ')}
                </span>
              </li>
            ))}
          </ul>
          <button
            type="button"
            className="btn secondary"
            onClick={() => void onExport('csv', 'flaky')}
          >
            Export flaky CSV
          </button>
        </section>
      </div>
    </div>
  )
}
