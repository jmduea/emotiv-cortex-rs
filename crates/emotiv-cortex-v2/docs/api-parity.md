# Cortex API Parity Matrix (`emotiv-cortex-v2`)

Source of truth: <https://emotiv.gitbook.io/cortex-api>  
Validated against docs snapshot date: **2026-02-12**.

Status legend:
- `match`: crate request/response shape and semantics align with docs.
- `partial`: compatible for common cases, but docs and crate differ in optional fields/modes or need broader validation.
- `mismatch`: known incompatibility requiring code change before release.

| Cortex method | GitBook docs | Crate method(s) | Status | Notes |
|---|---|---|---|---|
| `getCortexInfo` | <https://emotiv.gitbook.io/cortex-api/cortex/getcortexinfo> | `CortexClient::get_cortex_info`, `ResilientClient::get_cortex_info` | `match` | Health-check path. |
| `getUserLogin` | <https://emotiv.gitbook.io/cortex-api/authentication/getuserlogin> | `CortexClient::get_user_login`, `ResilientClient::get_user_login` | `match` | Typed deserialization in `UserLoginInfo`. |
| `requestAccess` | <https://emotiv.gitbook.io/cortex-api/authentication/requestaccess> | internal via `CortexClient::authenticate` | `match` | Gracefully handles unavailable method. |
| `hasAccessRight` | <https://emotiv.gitbook.io/cortex-api/authentication/hasaccessright> | `CortexClient::has_access_right`, `ResilientClient::has_access_right` | `match` | Returns `accessGranted` bool. |
| `authorize` | <https://emotiv.gitbook.io/cortex-api/authentication/authorize> | `CortexClient::authenticate` | `match` | Token extraction validated. |
| `generateNewToken` | <https://emotiv.gitbook.io/cortex-api/authentication/generatenewtoken> | `CortexClient::generate_new_token`, `ResilientClient::generate_new_token` | `match` | Updates resilient token state on success. |
| `getUserInformation` | <https://emotiv.gitbook.io/cortex-api/authentication/getuserinformation> | `CortexClient::get_user_info`, `ResilientClient::get_user_info` | `match` | Raw JSON passthrough. |
| `getLicenseInfo` | <https://emotiv.gitbook.io/cortex-api/authentication/getlicenseinfo> | `CortexClient::get_license_info`, `ResilientClient::get_license_info` | `match` | Raw JSON passthrough. |
| `controlDevice` | <https://emotiv.gitbook.io/cortex-api/headset/controldevice> | `connect_headset`, `disconnect_headset`, `refresh_headsets` (+ resilient wrappers) | `match` | Uses documented `command` values. |
| `configMapping` | <https://emotiv.gitbook.io/cortex-api/headset/configmapping> | `config_mapping` (+ resilient wrapper) | `partial` | Needs broader mode-by-mode payload validation. |
| `queryHeadsets` | <https://emotiv.gitbook.io/cortex-api/headset/queryheadsets> | `query_headsets` (+ resilient wrapper) | `partial` | Compatible; additional optional fields continue to evolve. |
| `updateHeadset` | <https://emotiv.gitbook.io/cortex-api/headset/updateheadset> | `update_headset` (+ resilient wrapper) | `match` | Uses `headset` and `setting`. |
| `updateHeadsetCustomInfo` | <https://emotiv.gitbook.io/cortex-api/headset/updateheadsetcustominfo> | `update_headset_custom_info` (+ resilient wrapper) | `match` | Uses `headsetId` per docs; retains compatibility field. |
| `syncWithHeadsetClock` | <https://emotiv.gitbook.io/cortex-api/headset/syncwithheadsetclock> | `sync_with_headset_clock` (+ resilient wrapper) | `partial` | Endpoint available; payload compatibility retained for deployed variants. |
| `createSession` | <https://emotiv.gitbook.io/cortex-api/session/createsession> | `create_session` (+ resilient wrapper) | `match` | Uses `status: "active"`. |
| `updateSession` | <https://emotiv.gitbook.io/cortex-api/session/updatesession> | `close_session` (+ resilient wrapper) | `match` | Close now propagates API errors. |
| `querySessions` | <https://emotiv.gitbook.io/cortex-api/session/querysessions> | `query_sessions` (+ resilient wrapper) | `match` | Typed deserialization in `SessionInfo`. |
| `subscribe` | <https://emotiv.gitbook.io/cortex-api/data-subscription/subscribe> | `subscribe_streams`, `streams::subscribe_*`, resilient wrappers | `match` | All known stream names covered. |
| `unsubscribe` | <https://emotiv.gitbook.io/cortex-api/data-subscription/unsubscribe> | `unsubscribe_streams`, `streams::unsubscribe`, resilient wrapper | `match` | Channel cleanup helper included. |
| `createRecord` | <https://emotiv.gitbook.io/cortex-api/records/createrecord> | `create_record` (+ resilient wrapper) | `match` | Extracts `record` envelope. |
| `stopRecord` | <https://emotiv.gitbook.io/cortex-api/records/stoprecord> | `stop_record` (+ resilient wrapper) | `match` | Extracts `record` envelope. |
| `updateRecord` | <https://emotiv.gitbook.io/cortex-api/records/updaterecord> | `update_record` (+ resilient wrapper) | `match` | Optional title/description/tags. |
| `deleteRecord` | <https://emotiv.gitbook.io/cortex-api/records/deleterecord> | `delete_record` (+ resilient wrapper) | `match` | Raw JSON result passthrough. |
| `exportRecord` | <https://emotiv.gitbook.io/cortex-api/records/exportrecord> | `export_record` (+ resilient wrapper) | `match` | Supports CSV and EDF. |
| `queryRecords` | <https://emotiv.gitbook.io/cortex-api/records/queryrecords> | `query_records` (+ resilient wrapper) | `match` | Parses `records` field, supports pagination. |
| `getRecordInfos` | <https://emotiv.gitbook.io/cortex-api/records/getrecordinfos> | `get_record_infos` (+ resilient wrapper) | `match` | Raw JSON passthrough. |
| `configOptOut` | <https://emotiv.gitbook.io/cortex-api/records/configoptout> | `config_opt_out` (+ resilient wrapper) | `match` | Supports `get` and `set`. |
| `requestToDownloadRecordData` | <https://emotiv.gitbook.io/cortex-api/records/requesttodownloadrecorddata> | `download_record` (+ resilient wrapper) | `match` | Method name mapped in `Methods`. |
| `injectMarker` | <https://emotiv.gitbook.io/cortex-api/markers/injectmarker> | `inject_marker` (+ resilient wrapper) | `match` | Supports explicit or generated timestamp. |
| `updateMarker` | <https://emotiv.gitbook.io/cortex-api/markers/updatemarker> | `update_marker` (+ resilient wrapper) | `match` | Interval marker conversion path. |
| `createSubject` | <https://emotiv.gitbook.io/cortex-api/subjects/createsubject> | `create_subject` (+ resilient wrapper) | `match` | Optional demographics and attributes. |
| `updateSubject` | <https://emotiv.gitbook.io/cortex-api/subjects/updatesubject> | `update_subject` (+ resilient wrapper) | `match` | Mirrors create subject payload shape. |
| `deleteSubjects` | <https://emotiv.gitbook.io/cortex-api/subjects/deletesubjects> | `delete_subjects` (+ resilient wrapper) | `match` | Raw JSON passthrough. |
| `querySubjects` | <https://emotiv.gitbook.io/cortex-api/subjects/querysubjects> | `query_subjects` (+ resilient wrapper) | `match` | Returns `(subjects, count)`. |
| `getDemographicAttributes` | <https://emotiv.gitbook.io/cortex-api/subjects/getdemographicattributes> | `get_demographic_attributes` (+ resilient wrapper) | `match` | Typed `DemographicAttribute`. |
| `queryProfile` | <https://emotiv.gitbook.io/cortex-api/profiles/queryprofile> | `query_profiles` (+ resilient wrapper) | `partial` | Response model now accepts current optional fields. |
| `getCurrentProfile` | <https://emotiv.gitbook.io/cortex-api/profiles/getcurrentprofile> | `get_current_profile` (+ resilient wrapper) | `partial` | Handles null/empty profile states compatibly. |
| `setupProfile` | <https://emotiv.gitbook.io/cortex-api/profiles/setupprofile> | `setup_profile` (+ resilient wrapper) | `match` | `ProfileAction` maps to status string. |
| `loadGuestProfile` | <https://emotiv.gitbook.io/cortex-api/profiles/loadguestprofile> | `load_guest_profile` (+ resilient wrapper) | `match` | Token-managed wrapper available. |
| `training` | <https://emotiv.gitbook.io/cortex-api/bci/training> | `training` (+ resilient wrapper) | `match` | `DetectionType` and `TrainingStatus` mapped. |
| `getDetectionInfo` | <https://emotiv.gitbook.io/cortex-api/bci/getdetectioninfo> | `get_detection_info` (+ resilient wrapper) | `match` | Typed `DetectionInfo`. |
| `getTrainedSignatureActions` | <https://emotiv.gitbook.io/cortex-api/advanced-bci/gettrainedsignatureactions> | `get_trained_signature_actions` (+ resilient wrapper) | `match` | Supports profile or session target. |
| `getTrainingTime` | <https://emotiv.gitbook.io/cortex-api/advanced-bci/gettrainingtime> | `get_training_time` (+ resilient wrapper) | `match` | Typed `TrainingTime`. |
| `facialExpressionSignatureType` | <https://emotiv.gitbook.io/cortex-api/advanced-bci/facialexpressionsignaturetype> | `facial_expression_signature_type` (+ resilient wrapper) | `match` | Supports get/set path. |
| `facialExpressionThreshold` | <https://emotiv.gitbook.io/cortex-api/advanced-bci/facialexpressionthreshold> | `facial_expression_threshold` (+ resilient wrapper) | `match` | Supports get/set path and value. |
| `mentalCommandActiveAction` | <https://emotiv.gitbook.io/cortex-api/advanced-bci/mentalcommandactiveaction> | `mental_command_active_action` (+ resilient wrapper) | `match` | Supports get/set with actions list. |
| `mentalCommandBrainMap` | <https://emotiv.gitbook.io/cortex-api/advanced-bci/mentalcommandbrainmap> | `mental_command_brain_map` (+ resilient wrapper) | `match` | Session-scoped query. |
| `mentalCommandTrainingThreshold` | <https://emotiv.gitbook.io/cortex-api/advanced-bci/mentalcommandtrainingthreshold> | `mental_command_training_threshold`, `mental_command_training_threshold_for_profile`, resilient wrappers | `match` | Session and profile paths both supported. |
| `mentalCommandActionSensitivity` | <https://emotiv.gitbook.io/cortex-api/advanced-bci/mentalcommandactionsensitivity> | `mental_command_action_sensitivity` (+ resilient wrapper) | `match` | Supports get/set with values. |

## Release criterion

No rows are currently marked `mismatch`.  
Rows marked `partial` are intentionally compatible but require continued API-doc drift checks during release validation.
