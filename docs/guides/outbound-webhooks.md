# Outbound webhooks

Project-scoped HTTPS endpoints receive signed JSON when cases, runs, or results change.

## Setup

1. Open **Settings → Outbound webhooks** (requires **manage CI config** on the project).
2. Create an endpoint: name, public HTTPS URL, and event checkboxes.
3. Copy the signing secret when shown — it is not returned again (use **rotate secret** on update to mint a new one).

## Events

| Event | When |
| --- | --- |
| `case.created` | Case created |
| `case.updated` | Case updated |
| `case.deleted` | Case deleted |
| `run.created` | Run created |
| `run.updated` | Run updated |
| `result.created` | Batch ingest inserted results (summary: `inserted`, `run_id`) |
| `webhook.test` | Test button only |

## Delivery

POST body:

```json
{
  "id": "<delivery-uuid>",
  "event": "case.created",
  "created_at": "2026-11-17T09:00:00Z",
  "project_id": "<uuid>",
  "data": { }
}
```

Headers: `Content-Type: application/json`, `X-Vulyk-Signature: sha256=<hex>`, `X-Vulyk-Event`, `X-Vulyk-Delivery`, `User-Agent: Vulyk-Webhooks/0.6`.

Verify HMAC-SHA256 over the raw body with the endpoint secret (same `sha256=` format as GitHub webhooks).

Retries use the shared jobs queue (up to 8 attempts). **4xx** responses (except **429**) mark the delivery failed without further retries. View deliveries per endpoint and **redeliver** from Settings or `POST /api/v1/projects/{project_id}/outbound-webhooks/deliveries/{delivery_id}/redeliver`.

Outbound URLs follow the same SSRF/egress policy as CI and automation remotes (HTTPS; private hosts only in lab with `VULYK_EGRESS_ALLOW_PRIVATE=1`).

## API

See OpenAPI tag `outbound-webhooks` in `/api/docs` or [API user guide](../user/api.md).