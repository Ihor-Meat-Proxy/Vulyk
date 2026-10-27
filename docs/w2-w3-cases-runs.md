# W2 + W3 — cases & runs (archive)

> **Superseded for readers.** Case repository depth and plan/run execution are
> documented in the product guides. This workstream label remains for historical
> cross-links from older PRs.

| Topic | Canonical doc |
|-------|----------------|
| Folders, keys, columns, PDF export, editing | [user-guide.md](user-guide.md) |
| Plans, runs, result history, statuses | [user-guide.md](user-guide.md#plans--runs) |
| Option lists & custom fields (admin + values) | [admin-guide.md](admin-guide.md#lists-custom-fields--config-export) |
| HTTP surfaces (incl. PDF `case_ids` max 100) | [api.md](api.md) |
| Architecture / ERD | [architecture.md](architecture.md) |

Custom field **definitions** are managed under Admin (global or selected projects)
with option lists; project editors set **values** on cases and requirements.
The older project-only `CRUD /projects/{id}/custom-fields` path is not the
primary admin model — see Admin → Lists / Custom fields / Export·import.
