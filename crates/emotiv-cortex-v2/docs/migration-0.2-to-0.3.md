# Migration Guide: `0.2.x` -> `0.3.0`

This release is a breaking redesign focused on transport safety and request-DTO APIs.

## Baseline changes

- MSRV raised to `1.85`
- Rust edition updated to `2024`
- Crate versions:
  - `emotiv-cortex-v2 = 0.3.0`
  - `emotiv-cortex-tui = 0.3.0`

## Method mapping (old -> new)

| Area | Old API | New API |
|---|---|---|
| Records | `update_record(token, record_id, title, description, tags)` | `update_record_with(token, &UpdateRecordRequest)` |
| Subjects | `create_subject(token, name, dob, sex, country, state, city, attrs)` | `create_subject_with(token, &SubjectRequest)` |
| Subjects | `update_subject(token, name, dob, sex, country, state, city, attrs)` | `update_subject_with(token, &SubjectRequest)` |
| Subjects | `query_subjects(token, query, order_by, limit, offset)` | `query_subjects_with(token, &QuerySubjectsRequest)` |
| Training | `mental_command_training_threshold_with_params(token, session, profile, status, value)` | `mental_command_training_threshold_with_request(token, &MentalCommandTrainingThresholdRequest)` |
| Training | `facial_expression_signature_type(token, status, profile, session, signature)` | `facial_expression_signature_type_with(token, &FacialExpressionSignatureTypeRequest)` |
| Training | `facial_expression_threshold(token, status, action, profile, session, value)` | `facial_expression_threshold_with(token, &FacialExpressionThresholdRequest)` |

`ResilientClient` exposes the same `*_with` request-based methods:

- `update_record_with(&UpdateRecordRequest)`
- `create_subject_with(&SubjectRequest)`
- `update_subject_with(&SubjectRequest)`
- `query_subjects_with(&QuerySubjectsRequest)`
- `mental_command_training_threshold_with_request(&MentalCommandTrainingThresholdRequest)`
- `facial_expression_signature_type_with(&FacialExpressionSignatureTypeRequest)`
- `facial_expression_threshold_with(&FacialExpressionThresholdRequest)`

## Compatibility window

Old long-argument methods remain available as deprecated wrappers in `0.3.0` to support staged migration.

## Transport behavior changes

- Pending RPC entries are now cleaned synchronously on send failure and timeout.
- Reader shutdown is signal-driven (non-polling) and drains pending RPC waiters.
- Stream dispatch now surfaces per-stream counters:
  - `delivered`
  - `dropped_full`
  - `dropped_closed`

## Example rewrite

```rust
use emotiv_cortex_v2::protocol::records::UpdateRecordRequest;

let request = UpdateRecordRequest {
    record_id: "record-1".into(),
    title: Some("New title".into()),
    description: None,
    tags: None,
};
client.update_record_with(token, &request).await?;
```
