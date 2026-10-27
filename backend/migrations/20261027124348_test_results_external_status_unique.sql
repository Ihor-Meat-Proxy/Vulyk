-- Review #6: make (run_id, external_id, status) unique for non-empty external_id so
-- concurrent ingest cannot insert duplicate idempotent replays.
-- Append-only history still allows the same external_id with a *different* status.

-- Drop exact duplicate history rows (keep the oldest id).
DELETE FROM test_results a
    USING test_results b
WHERE a.external_id IS NOT NULL
  AND btrim(a.external_id) <> ''
  AND a.external_id = b.external_id
  AND a.run_id = b.run_id
  AND a.status = b.status
  AND a.id > b.id;

CREATE UNIQUE INDEX IF NOT EXISTS test_results_run_external_status_uidx
    ON test_results (run_id, external_id, status)
    WHERE external_id IS NOT NULL AND btrim(external_id) <> '';
