# Vulyk QA feature seed fragments

Parallel PRs that each append cases to `../vulyk-qa.json` (or
`../generate_vulyk_qa.py`) often collide on hardcoded `VQA-NNN` keys after
rebase. **Prefer a fragment file here instead.**

## How to add cases for a feature PR

1. Create `backend/seed/vulyk-qa.d/<feature-slug>.json` (kebab-case slug).
2. Omit `"key"` on new cases (or leave it `""`). Keys are assigned at seed /
   check time as the next free `VQA-N` after the base file and other fragments.
3. Register the fragment in `backend/src/sample_seed.rs` (`VULYK_QA_FRAGMENTS`).
4. Run `./scripts/check-seed-keys.sh` (also part of `./scripts/check.sh`).

## Fragment schema

```json
{
  "feature": "pdf-export",
  "suites": [
    {
      "name": { "en": "PDF export", "uk": "Експорт PDF" },
      "children": [],
      "cases": [
        {
          "title": { "en": "Export one case as PDF", "uk": "…" },
          "description": { "en": "…", "uk": "…" },
          "precondition": { "en": "…", "uk": "…" },
          "priority": "medium",
          "status": "ready",
          "tags": ["pdf", "export"],
          "steps": [
            {
              "action": { "en": "Open a case", "uk": "…" },
              "expected_result": { "en": "Export control is visible", "uk": "…" },
              "data": { "en": "", "uk": "" }
            }
          ]
        }
      ]
    }
  ]
}
```

Optional top-level `requirements` / `runs` are merged into the base seed the
same way. Do **not** invent parallel numeric keys in the base JSON for the same
feature — that is what caused CI collisions on PRs #27–#33.

## Rules

- Fragment `feature` must be unique across files.
- Explicit keys, if present, must be unique across base + all fragments and
  match `VQA-<digits>` (project prefix `VQA`).
- Prefer omitting keys so allocation stays collision-proof under parallel PRs.
- After rebase onto latest `main`, re-run `./scripts/check-seed-keys.sh`.
- Exception: `playwright-e2e.json` keeps stable `VQA-N` keys so Playwright
  `@VQA-N` tags stay aligned; regenerate with
  `node e2e/scripts/sync-vulyk-case-links.mjs --annotate` after rebase if keys
  collide, then update tags.
