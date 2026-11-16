# Import

Bring cases into a Vulyk project from other test tools or a generic CSV file. Open **Import** at `/import`. You need the **import cases** permission (project Admin or Test Architect).

## Supported importers

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

The Import page loads this registry from the API (`GET /api/v1/importers`) and shows only adapters your server exposes.

## How import works

1. Choose the **importer** and **format**.
2. Paste or upload the file **content**.
3. Review the **field mapping** (`title`, `key`, `description`, `precondition`, `priority`, `status`, `suite`, `steps`). Defaults come from the importer; edit them to match your export columns or keys.
4. Click **Preview** (dry-run). The API parses and maps without writing. Review the preview list (first rows), total count, and any warnings.
5. When the mapping looks correct, run the real import. Vulyk creates folders/suites, cases, and steps as needed.

Imported case **keys** are allocated from the project counter (`PREFIX-N`). Original vendor IDs are stored as `external_key` when provided.

## Field mapping tips

- Map the source title column/key to `title` (required for useful cases).
- Map suite/folder paths to `suite` so the folder tree mirrors the source tool.
- Map step structures when the format includes them; otherwise add steps after import.
- Custom fields defined in Vulyk can be targeted when the importer/UI mapping exposes them — align names carefully before the final import.

## Dry-run vs commit

| Action | Effect |
|--------|--------|
| Preview / `dry_run: true` | Parse + map only; no database writes |
| Import / `dry_run: false` | Persist suites, cases, and steps |

Always preview large exports first. Fix mapping warnings before committing.

## After import

- Find cases under **Cases** by folder or search.
- Link requirements and set tags/custom fields as needed.
- For automation, put Vulyk keys into your tests (title, annotation, or JUnit `test_key`) before CI ingest — see [CI/CD](cicd.md).

## Config export / import (not case import)

**Admin → Export / import** is a different feature: versioned JSON/YAML **configuration** bundles (fields, lists, settings; secrets redacted), optionally with case/requirement/run data. That path is for operators moving instance config, not day-to-day case migration from TestRail/Xray. See [Admin](admin.md).
