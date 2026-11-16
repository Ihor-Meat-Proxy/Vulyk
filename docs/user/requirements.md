# Requirements

Track product or regulatory requirements and link them to test cases for coverage reporting.

## Open requirements

1. Select a project in the top bar.
2. Open **Requirements** (`/requirements`).
3. Browse the list. Each row can show an external key when the requirement came from an ALM sync or was given an `external_key`.

Creating and managing requirements requires the **manage requirements** permission (project Admin or Test Architect; system admins always can).

## Create a requirement

1. Enter a **title** in the create form.
2. Submit. The requirement appears in the project list.
3. Select it to view details and edit custom fields that apply to requirements (when an admin defined them).

Custom field types and scopes follow the same model as for cases (Admin → Lists / Custom fields). Save custom field values on the selected requirement when you have permission.

## Link requirements to cases (traceability)

Traceability is maintained from the **case detail** page:

1. Open a case (`/cases/{KEY}`).
2. On the Details tab, find **Linked requirements**.
3. Choose a requirement from the dropdown and click **Link**.

Linked requirements appear on the case, in case **PDF export**, and feed **Reports → coverage**. Unlinked requirements show as uncovered in the coverage views.

You need **manage requirements** to link. Viewers and other roles can still read coverage where they have **view reports**.

## Coverage reports

1. Open **Reports** (`/reports`).
2. Use requirement coverage cards / KPIs, or go to `/reports/coverage`.
3. Review covered vs uncovered requirements, linked cases, and latest status.
4. Export CSV or PDF when you have **export reports** permission.

Coverage is the primary way stakeholders see which requirements are exercised by cases and how those cases are performing.

## ALM-synced requirements

When integrations are configured (Admin → Integrations), requirements may arrive with an **external key** from the ALM. Manual requirements work the same for linking and coverage; the badge shows the external key when present.

## AI proposals

From **Chat**, models with `llm_assist` can **propose requirements** (title, description, acceptance criteria). Confirming create requires roles that may create requirements (Admin / Test Architect). Nothing is written until you review the preview card and click **Create**. See [AI assist](ai.md).
