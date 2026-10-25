# Case importers (W7)

Pluggable `Importer` trait (`backend/src/importers`) with a static registry.

| Id | Name | Formats |
|----|------|---------|
| `csv` | Generic CSV | csv |
| `xray` | Jira Xray | json, csv |
| `zephyr` | Zephyr Scale / Squad | json |
| `squash` | Squash TM | json, xml |
| `testlink` | TestLink | xml |
| `testrail` | TestRail | csv, xml, json, api |
| `qase` | Qase | json |
| `qtest` | qTest | json |
| `kiwi` | Kiwi TCMS | json |

## Field mapping

Clients send a `field_mapping` object (`title`, `key`, `description`, `precondition`, `priority`, `status`, `suite`, `steps`) mapping source columns/keys to Vulyk fields. Defaults are importer-specific (`default_mapping_for`).

## Dry-run preview

1. `GET /api/v1/importers` — list adapters  
2. `POST /api/v1/projects/{id}/imports:preview` — parse + map, no writes  
3. `POST /api/v1/projects/{id}/imports` with `dry_run: true` — same as preview  
4. `POST /api/v1/projects/{id}/imports` with `dry_run: false` — persist suites/cases/steps  

UI: **Import** page (`/import`) — choose importer, paste content, edit mapping, preview, then import.

## Fixtures

Parser tests load files under `backend/tests/fixtures/importers/` (no live vendor APIs).
