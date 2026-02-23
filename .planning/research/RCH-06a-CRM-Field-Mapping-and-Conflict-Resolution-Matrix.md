# RCH-06a: CRM Field-Mapping and Conflict-Resolution Matrix

**Bead:** `bd-3d8.11.7.1`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** IvoryBear (Codex)

## 1. Objective

Provide an implementation-ready CRM mapping workbook that includes:

1. canonical field mapping across internal domain and CRM providers,
2. explicit nullability and versioning semantics,
3. deterministic conflict policy matrix,
4. retry-safe write behaviors for outbound operations.

## 2. Inputs and Constraints

Primary references:

1. `.planning/research/RCH-06-CRM-Contract-and-Sync-Boundary-Design.md`
2. `.planning/research/RCH-04-Reliability-and-Idempotency-Architecture.md`
3. `.planning/PROJECT.md`

Constraints carried forward:

1. Core CPQ authority remains local-first in SQLite.
2. CRM integration is adapter-based (`StubCrmAdapter`, `ComposioCrmAdapter`).
3. Sync is eventually consistent and must not break quote lifecycle determinism.

## 3. Canonical Mapping Schema (Fixture Shape)

Recommended mapping row shape:

| Field | Type | Description |
|---|---|---|
| `map_id` | string | Stable mapping identifier (`CRM-MAP-*`). |
| `entity` | enum | `account`, `contact`, `deal`, `quote_summary`. |
| `domain_field` | string | Internal canonical field path. |
| `salesforce_field` | string nullable | Salesforce target field API name/path. |
| `hubspot_field` | string nullable | HubSpot target property/path. |
| `direction` | enum | `inbound`, `outbound`, `bidirectional`. |
| `required` | bool | Required for contract validity. |
| `nullable` | bool | Whether null is accepted after normalization. |
| `default_value` | string nullable | Default for absent optional values. |
| `type_contract` | string | Expected normalized type (`string`, `decimal`, `utc_ts`, etc.). |
| `owner_policy` | enum | `crm_wins`, `local_wins`, `merge_with_audit`, `immutable_after_bind`. |
| `version_semantics` | enum | `etag`, `updated_at`, `revision`, `none`. |
| `retry_behavior` | enum | `safe_retry`, `retry_with_reconcile`, `no_retry_terminal`. |
| `notes` | string nullable | Provider-specific caveats. |

## 4. Canonical Field Mapping Workbook

### 4.1 Account Mapping

| Map ID | Domain Field | Salesforce | HubSpot | Direction | Required | Nullable | Owner | Version Semantics | Retry Behavior |
|---|---|---|---|---|---|---|---|---|---|
| `CRM-MAP-001` | `account.crm_ref` | `Account.Id` | `company.hs_object_id` | inbound | yes | no | `immutable_after_bind` | `etag`/`updated_at` | `no_retry_terminal` on mismatch |
| `CRM-MAP-002` | `account.name` | `Account.Name` | `company.name` | bidirectional | yes | no | `crm_wins` | `updated_at` | `retry_with_reconcile` |
| `CRM-MAP-003` | `account.domain` | `Account.Website` | `company.domain` | inbound | no | yes | `crm_wins` | `updated_at` | `safe_retry` |
| `CRM-MAP-004` | `account.segment` | `Account.Segment__c` | `company.segment` | bidirectional | no | yes | `merge_with_audit` | `updated_at` | `retry_with_reconcile` |

### 4.2 Contact Mapping

| Map ID | Domain Field | Salesforce | HubSpot | Direction | Required | Nullable | Owner | Version Semantics | Retry Behavior |
|---|---|---|---|---|---|---|---|---|---|
| `CRM-MAP-010` | `contact.crm_ref` | `Contact.Id` | `contact.hs_object_id` | inbound | yes | no | `immutable_after_bind` | `etag`/`updated_at` | `no_retry_terminal` |
| `CRM-MAP-011` | `contact.email` | `Contact.Email` | `contact.email` | bidirectional | yes | no | `crm_wins` | `updated_at` | `retry_with_reconcile` |
| `CRM-MAP-012` | `contact.full_name` | `Contact.Name` | `contact.firstname+lastname` | inbound | yes | no | `crm_wins` | `updated_at` | `safe_retry` |
| `CRM-MAP-013` | `contact.role_hint` | `Contact.Title` | `contact.jobtitle` | inbound | no | yes | `crm_wins` | `updated_at` | `safe_retry` |

### 4.3 Deal Mapping

| Map ID | Domain Field | Salesforce | HubSpot | Direction | Required | Nullable | Owner | Version Semantics | Retry Behavior |
|---|---|---|---|---|---|---|---|---|---|
| `CRM-MAP-020` | `deal.crm_ref` | `Opportunity.Id` | `deal.hs_object_id` | inbound | yes | no | `immutable_after_bind` | `etag`/`updated_at` | `no_retry_terminal` |
| `CRM-MAP-021` | `deal.account_ref` | `Opportunity.AccountId` | `deal.associations.company` | inbound | yes | no | `crm_wins` | `updated_at` | `no_retry_terminal` on missing parent |
| `CRM-MAP-022` | `deal.name` | `Opportunity.Name` | `deal.dealname` | bidirectional | yes | no | `crm_wins` | `updated_at` | `retry_with_reconcile` |
| `CRM-MAP-023` | `deal.stage` | `Opportunity.StageName` | `deal.dealstage` | inbound | yes | no | `crm_wins` | `revision` | `retry_with_reconcile` |
| `CRM-MAP-024` | `deal.close_date` | `Opportunity.CloseDate` | `deal.closedate` | inbound | no | yes | `crm_wins` | `updated_at` | `safe_retry` |
| `CRM-MAP-025` | `deal.amount_hint` | `Opportunity.Amount` | `deal.amount` | inbound | no | yes | `merge_with_audit` | `updated_at` | `retry_with_reconcile` |

### 4.4 Quote Summary Writeback Mapping

| Map ID | Domain Field | Salesforce | HubSpot | Direction | Required | Nullable | Owner | Version Semantics | Retry Behavior |
|---|---|---|---|---|---|---|---|---|---|
| `CRM-MAP-030` | `quote.id` | `Quote__c.Quotey_Id__c` | `deal.quotey_quote_id` | outbound | yes | no | `local_wins` | `revision` | `safe_retry` |
| `CRM-MAP-031` | `quote.version` | `Quote__c.Quotey_Version__c` | `deal.quotey_quote_version` | outbound | yes | no | `local_wins` | `revision` | `safe_retry` with stale guard |
| `CRM-MAP-032` | `quote.status` | `Quote__c.Quotey_Status__c` | `deal.quotey_quote_status` | outbound | yes | no | `local_wins` | `revision` | `safe_retry` |
| `CRM-MAP-033` | `quote.total` | `Quote__c.Quotey_Total__c` | `deal.quotey_quote_total` | outbound | yes | no | `local_wins` | `revision` | `safe_retry` |
| `CRM-MAP-034` | `quote.currency` | `Quote__c.CurrencyIsoCode` | `deal.quotey_currency` | outbound | yes | no | `local_wins` | `none` | `safe_retry` |
| `CRM-MAP-035` | `quote.valid_until` | `Quote__c.Valid_Until__c` | `deal.quotey_valid_until` | outbound | no | yes | `local_wins` | `revision` | `safe_retry` |
| `CRM-MAP-036` | `quote.attachment_ref` | `ContentDocumentLink`/`Quote__c.Pdf_Ref__c` | `deal.quotey_pdf_ref` | outbound | no | yes | `local_wins` | `revision` | `retry_with_reconcile` |

## 5. Nullability and Normalization Semantics

Normalization rules:

1. Required non-null fields missing after provider transform are terminal mapping errors.
2. Optional nullable fields may be absent and normalized to `null`.
3. Empty string from provider is normalized to `null` for fields marked nullable.
4. Numeric parse failure for required numeric fields is terminal (`no_retry_terminal`).
5. UTC timestamps must parse to normalized `RFC3339`; invalid values cause mapping rejection.

Contract checks:

1. `domain_field` required + non-null invariants are enforced before domain mutation.
2. Provider-specific fallback rules are allowed only for optional fields.
3. Nullability changes require policy snapshot bump and migration note.

## 6. Versioning Semantics

Canonical version strategy by data class:

1. Identity bindings (`crm_ref`): immutable after bind; mismatch is conflict.
2. Process fields (`deal.stage`, `close_date`): use remote `updated_at`/revision semantics.
3. Local quote writeback: use local `quote.version` monotonic revision.

Version conflict rules:

1. Outbound write must include `quote.version`.
2. If remote version marker is newer than outbound `quote.version`, classify as stale write conflict.
3. Inbound updates older than local mirror revision are ignored with audit event.
4. Missing provider version metadata falls back to deterministic timestamp comparison with tie-break on source priority.

## 7. Conflict-Resolution Policy Matrix

| Conflict ID | Conflict Class | Detection Rule | Resolution | Retry Policy | Audit Consequence |
|---|---|---|---|---|---|
| `CRM-CFL-001` | stale outbound quote write | remote `quote_version` > outbound `quote.version` | reject outbound write; create reconciliation item | `no_retry_terminal` | `crm.sync_conflict_stale_outbound` |
| `CRM-CFL-002` | immutable ID mismatch | inbound `crm_ref` differs from bound local `crm_ref` | reject inbound and quarantine record | `no_retry_terminal` | `crm.sync_conflict_identity_mismatch` |
| `CRM-CFL-003` | crm_wins field divergence | inbound value differs on `crm_wins` field | apply inbound value, record prior local value | `safe_retry` | `crm.sync_applied_crm_wins` |
| `CRM-CFL-004` | local_wins field divergence | outbound value differs on `local_wins` field | keep local, push outbound summary | `retry_with_reconcile` on transient failures | `crm.sync_applied_local_wins` |
| `CRM-CFL-005` | merge_with_audit divergence | both sides changed since last sync | deterministic merge policy then persist loser snapshot | `retry_with_reconcile` | `crm.sync_merged_with_audit` |
| `CRM-CFL-006` | schema/type mismatch | provider value fails type contract | do not mutate; mark mapping terminal error | `no_retry_terminal` until mapping fix | `crm.sync_mapping_error` |
| `CRM-CFL-007` | association missing | deal/account parent missing remotely | queue reconciliation and retry lookup path | `retry_with_reconcile` | `crm.sync_missing_association` |
| `CRM-CFL-008` | concurrent bidirectional update | same field changed both sides within sync window | apply owner policy, require lineage note | `retry_with_reconcile` | `crm.sync_conflict_bidirectional` |

## 8. Retry-Safe Write Behavior Matrix

| Operation | Idempotency Key Dimensions | Safe Retry Condition | Terminal Condition | Reconcile Requirement |
|---|---|---|---|---|
| `create_deal` | `source`, `account_ref`, `normalized_payload_hash` | timeout/5xx/rate-limit | mapping validation failure | yes, if uncertain remote creation |
| `update_deal` | `deal.crm_ref`, `field_set_hash`, `quote.version` | timeout/5xx/rate-limit with unchanged local snapshot | immutable mismatch or schema error | yes |
| `write_quote_summary` | `deal.crm_ref`, `quote.id`, `quote.version` | transient network/provider errors | stale outbound version conflict | yes |
| `attach_quote_artifact` | `deal.crm_ref`, `quote.id`, `artifact_checksum` | upload timeout/5xx | missing artifact reference or auth hard-fail | yes |
| `sync_incremental` | `cursor`, `provider`, `window_start` | transient pull failures | invalid cursor contract | yes |

Retry safety rules:

1. Every outbound write includes deterministic operation key and policy snapshot reference.
2. Retried operation must not mutate local quote state beyond sync metadata.
3. Terminal errors remain visible until explicit reconciliation or mapping update.

## 9. Reconciliation Queue Contract

Minimum reconciliation record:

1. `reconcile_id`
2. `entity`
3. `entity_ref`
4. `field_name`
5. `local_value`
6. `remote_value`
7. `owner_policy`
8. `conflict_id`
9. `first_seen_ts`
10. `last_attempt_ts`
11. `correlation_id`

Operator action outcomes:

1. `accept_local`
2. `accept_remote`
3. `merge_with_note`
4. `defer_with_reason`

## 10. Starter Fixture Snippet

```json
[
  {
    "map_id": "CRM-MAP-031",
    "entity": "quote_summary",
    "domain_field": "quote.version",
    "salesforce_field": "Quote__c.Quotey_Version__c",
    "hubspot_field": "deal.quotey_quote_version",
    "direction": "outbound",
    "required": true,
    "nullable": false,
    "owner_policy": "local_wins",
    "version_semantics": "revision",
    "retry_behavior": "safe_retry"
  },
  {
    "map_id": "CRM-MAP-023",
    "entity": "deal",
    "domain_field": "deal.stage",
    "salesforce_field": "Opportunity.StageName",
    "hubspot_field": "deal.dealstage",
    "direction": "inbound",
    "required": true,
    "nullable": false,
    "owner_policy": "crm_wins",
    "version_semantics": "revision",
    "retry_behavior": "retry_with_reconcile"
  }
]
```

## 11. Acceptance Criteria Mapping

`bd-3d8.11.7.1` requirements:

1. **Includes nullability/versioning semantics**: Sections 5 and 6.
2. **Covers retry-safe write behaviors**: Sections 8 and 9.

This matrix is ready for adapter-fixture and reconciliation implementation tasks.
