# OpenProject integration

Vulyk integrates with **OpenProject** as an external ALM via [APIv3](https://www.openproject.org/docs/api/). This repository does **not** open PRs, issues, or commits against OpenProject upstream—only documents and implements the client contract here.

## Mapping

| Vulyk concept | OpenProject |
|---------------|-------------|
| Requirement | Work package (type from `config.requirement_type_id`) |
| Defect | Work package (type from `config.defect_type_id`) |
| Case ↔ requirement link | Stored in Vulyk `case_requirements`; optional WP custom field via `config.link_custom_field` |

## Configuration

Create an integration with `kind: "openproject"`:

| Field | Location | Description |
|-------|----------|-------------|
| `base_url` | row | e.g. `https://openproject.example.com` |
| `api_token` | `secrets` | User API token (Basic `apikey:<token>`) |
| `project_id` | `config` | OpenProject project id / identifier |
| `requirement_type_id` | `config` | Work package type for requirements |
| `defect_type_id` | `config` | Work package type for bugs |
| `link_custom_field` | `config` | Optional CF name written on link |
| `fixture_requirements` / `fixture_defect` | `config` | Test-only fixtures (no network) |

## API (Vulyk)

- `GET /api/v1/projects/{id}/integrations/{integrationId}/requirements:search?q=`
- `POST /api/v1/projects/{id}/integrations/{integrationId}/requirements:import`
- `POST /api/v1/projects/{id}/defects` with `integration_id` (+ optional `result_id` from a failed result)

## Adapter

`backend/src/integrations/openproject.rs` implements `AlmIntegration`.
