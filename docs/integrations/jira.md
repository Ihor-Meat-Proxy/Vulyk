# Jira Cloud and Jira Server / Data Center

Vulyk ships two ALM adapters that share issue DTO mapping but differ in auth and API version.

| Kind | Auth | Search / create |
|------|------|-----------------|
| `jira_cloud` | OAuth bearer (`secrets.oauth_access_token`) **or** email + API token (Basic) | REST API v3 |
| `jira_dc` | PAT bearer (`secrets.pat`) **or** username + password (Basic) | REST API v2 |

No credentials are committed; tests use `config.fixture_requirements` / `config.fixture_defect` only.

## Configuration

Common `config`:

- `project_key` — Jira project key
- `requirement_issue_types` — default `Story,Epic`
- `defect_issue_type` — default `Bug`

Cloud secrets: `oauth_access_token` **or** `email` + `api_token`.  
Server/DC secrets: `pat` **or** `username` + `password`.

## Behaviour

- **List / import requirements** — JQL search; upserts into Vulyk `requirements` with `source` = integration kind.
- **Link** — Vulyk stores `case_requirements`; adapters may post a remote comment (skipped in fixture mode).
- **Create defect** — opens a Bug (or configured type) from a failed `test_results` row via `POST .../defects`.

## Docs / code

- Cloud: `backend/src/integrations/jira_cloud.rs`
- Server/DC: `backend/src/integrations/jira_dc.rs`
