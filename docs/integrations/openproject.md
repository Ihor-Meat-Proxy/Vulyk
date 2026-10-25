# OpenProject integration (planned)

Vulyk integrates with **OpenProject** as an external ALM. This repository does **not** contribute code, PRs, or commits to OpenProject upstream.

## Intended use

| Vulyk concept | OpenProject mapping |
|---------------|---------------------|
| Requirement | Work package (type configurable) |
| Defect | Work package (bug type / custom type) |
| Link | External URL / custom field on both sides |

## Configuration (future admin UI)

- Base URL (e.g. `https://openproject.example.com`)
- API token (user-scoped)
- Project id / identifier
- Work package type ids for requirements and defects

## Adapter

Implements `AlmIntegration` in `backend` (workstream **W6**):

- `list_requirements` → OpenProject work packages API with filters
- `link_requirement` → store remote id on `requirements.external_key` + optional WP custom field
- `create_defect` → create work package with reproduction steps from failed result

API reference: [OpenProject APIv3](https://www.openproject.org/docs/api/).
