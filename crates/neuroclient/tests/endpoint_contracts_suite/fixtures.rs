use neuroclient::protocol::constants::{Methods, Streams};
use serde_json::{Value, json};

pub(super) const CLIENT_ID: &str = "test-client-id";
pub(super) const CLIENT_SECRET: &str = "test-client-secret";
pub(super) const TOKEN_CORTEX: &str = "token-contract";
pub(super) const TOKEN_RESILIENT: &str = "token-resilient";
pub(super) const SESSION_ID: &str = "session-001";
pub(super) const HEADSET_ID: &str = "HS-001";
pub(super) const PROFILE_NAME: &str = "profile-alpha";
pub(super) const RECORD_ID: &str = "record-1";
pub(super) const SUBJECT_NAME: &str = "subject-a";
pub(super) const FACIAL_ACTION: &str = "smile";
pub(super) const MAPPING_UUID: &str = "mapping-001";
pub(super) const MARKER_ID: &str = "marker-001";
pub(super) const MARKER_LABEL: &str = "stimulus";
pub(super) const HEADBAND_POSITION: &str = "left-temple";
pub(super) const CUSTOM_HEADSET_NAME: &str = "Contract Headset";

pub(super) const CONTRACT_STREAMS: [&str; 3] = [Streams::MOT, Streams::MET, Streams::COM];

#[derive(Debug, Clone)]
pub(super) enum StepKind {
    HasAccessRight,
    GetUserLogin,
    GetUserInfo,
    GetLicenseInfo,
    ConnectHeadset,
    DisconnectHeadset,
    RefreshHeadsets,
    ConfigMappingCreate,
    ConfigMappingGet,
    ConfigMappingRead,
    ConfigMappingUpdate,
    ConfigMappingDelete,
    UpdateHeadset,
    UpdateHeadsetCustomInfo,
    SyncWithHeadsetClock,
    CreateSession,
    CloseSession,
    QuerySessions,
    SubscribeStreams,
    UnsubscribeStreams,
    InjectMarker,
    UpdateMarker,
    CreateRecord,
    StopRecord,
    QueryRecords,
    ExportRecord,
    UpdateRecord,
    DeleteRecord,
    GetRecordInfos,
    ConfigOptOut,
    DownloadRecord,
    CreateSubject,
    UpdateSubject,
    DeleteSubjects,
    QuerySubjects,
    GetDemographicAttributes,
    QueryProfiles,
    GetCurrentProfile,
    SetupProfile,
    LoadGuestProfile,
    GetDetectionInfo,
    Training,
    MentalCommandActiveAction,
    MentalCommandActionSensitivity,
    MentalCommandBrainMap,
    MentalCommandTrainingThreshold,
    MentalCommandTrainingThresholdForProfile,
    MentalCommandTrainingThresholdWithParams,
    GetTrainedSignatureActions,
    GetTrainingTime,
    FacialExpressionSignatureType,
    FacialExpressionThreshold,
}

#[derive(Debug, Clone)]
pub(super) struct ContractStep {
    pub(super) domain: &'static str,
    pub(super) name: &'static str,
    pub(super) kind: StepKind,
    pub(super) method: &'static str,
    pub(super) expected_params: Value,
    pub(super) absent_params: Vec<&'static str>,
    pub(super) response: Value,
}

pub(super) fn record_ids() -> Vec<String> {
    vec!["record-1".to_string(), "record-2".to_string()]
}

pub(super) fn record_tags() -> Vec<String> {
    vec!["tag-a".to_string(), "tag-b".to_string()]
}

pub(super) fn subject_names() -> Vec<String> {
    vec!["subject-a".to_string(), "subject-b".to_string()]
}

pub(super) fn subject_attributes() -> Vec<Value> {
    vec![json!({
        "name": "handedness",
        "value": "right",
    })]
}

pub(super) fn subject_query() -> Value {
    json!({
        "subjectName": SUBJECT_NAME,
    })
}

pub(super) fn subject_order() -> Value {
    json!([{ "subjectName": "ASC" }])
}

pub(super) fn config_mapping_create_mappings() -> Value {
    json!({
        "CMS": "TP9",
        "DRL": "TP10",
    })
}

pub(super) fn config_mapping_updated_mappings() -> Value {
    json!({
        "CMS": "C3",
        "DRL": "C4",
    })
}

pub(super) fn session_response() -> Value {
    json!({
        "id": SESSION_ID,
        "status": "activated",
        "owner": "contract-user",
        "license": "license-001",
        "appId": "com.contract.test",
        "started": "2026-02-12T08:59:00Z",
        "streams": [Streams::MOT],
        "recordIds": [RECORD_ID],
        "recording": false,
        "headset": {
            "id": HEADSET_ID,
            "status": "connected",
        }
    })
}

// Flat data table of every endpoint contract; length reflects API surface,
// not logic complexity.
#[allow(clippy::too_many_lines)]
pub(super) fn build_contract_steps(token: &str) -> Vec<ContractStep> {
    vec![
        ContractStep {
            domain: "auth",
            name: "has_access_right",
            kind: StepKind::HasAccessRight,
            method: Methods::HAS_ACCESS_RIGHT,
            expected_params: json!({
                "clientId": CLIENT_ID,
                "clientSecret": CLIENT_SECRET,
            }),
            absent_params: vec!["cortexToken"],
            response: json!({
                "accessGranted": true,
                "message": "This application has been approved"
            }),
        },
        ContractStep {
            domain: "auth",
            name: "get_user_login",
            kind: StepKind::GetUserLogin,
            method: Methods::GET_USER_LOGIN,
            expected_params: json!({}),
            absent_params: vec!["cortexToken"],
            response: json!([
                {
                    "username": "contract-user",
                    "currentOSUId": "launcher",
                    "loggedInOSUId": "launcher",
                    "lastLoginTime": "2026-02-12T08:00:00Z"
                }
            ]),
        },
        ContractStep {
            domain: "auth",
            name: "get_user_info",
            kind: StepKind::GetUserInfo,
            method: Methods::GET_USER_INFO,
            expected_params: json!({
                "cortexToken": token,
            }),
            absent_params: vec![],
            response: json!({
                "username": "contract-user",
                "firstName": "Contract",
                "lastName": "Tester"
            }),
        },
        ContractStep {
            domain: "auth",
            name: "get_license_info",
            kind: StepKind::GetLicenseInfo,
            method: Methods::GET_LICENSE_INFO,
            expected_params: json!({
                "cortexToken": token,
            }),
            absent_params: vec![],
            response: json!({
                "isOnline": true,
                "license": {
                    "id": "license-001",
                    "scopes": ["eeg", "pm"]
                }
            }),
        },
        ContractStep {
            domain: "headset",
            name: "connect_headset",
            kind: StepKind::ConnectHeadset,
            method: Methods::CONTROL_DEVICE,
            expected_params: json!({
                "command": "connect",
                "headset": HEADSET_ID,
            }),
            absent_params: vec![],
            response: json!({
                "message": "connect issued"
            }),
        },
        ContractStep {
            domain: "headset",
            name: "disconnect_headset",
            kind: StepKind::DisconnectHeadset,
            method: Methods::CONTROL_DEVICE,
            expected_params: json!({
                "command": "disconnect",
                "headset": HEADSET_ID,
            }),
            absent_params: vec![],
            response: json!({
                "message": "disconnect issued"
            }),
        },
        ContractStep {
            domain: "headset",
            name: "refresh_headsets",
            kind: StepKind::RefreshHeadsets,
            method: Methods::CONTROL_DEVICE,
            expected_params: json!({
                "command": "refresh",
            }),
            absent_params: vec!["headset"],
            response: json!({
                "message": "refresh issued"
            }),
        },
        ContractStep {
            domain: "headset",
            name: "config_mapping_create",
            kind: StepKind::ConfigMappingCreate,
            method: Methods::CONFIG_MAPPING,
            expected_params: json!({
                "cortexToken": token,
                "status": "create",
                "name": "Flex Contract",
                "mappings": config_mapping_create_mappings(),
            }),
            absent_params: vec!["uuid"],
            response: json!({
                "message": "Create flex mapping config successful",
                "value": {
                    "label": {},
                    "mappings": config_mapping_create_mappings(),
                    "name": "Flex Contract",
                    "uuid": MAPPING_UUID
                }
            }),
        },
        ContractStep {
            domain: "headset",
            name: "config_mapping_get",
            kind: StepKind::ConfigMappingGet,
            method: Methods::CONFIG_MAPPING,
            expected_params: json!({
                "cortexToken": token,
                "status": "get",
            }),
            absent_params: vec!["uuid", "name", "mappings"],
            response: json!({
                "message": "Get flex mapping config successful",
                "value": {
                    "config": [{
                        "label": {},
                        "mappings": config_mapping_create_mappings(),
                        "name": "Flex Contract",
                        "uuid": MAPPING_UUID
                    }],
                    "updated": "2026-02-12T08:30:00Z",
                    "version": "2026-02-12"
                }
            }),
        },
        ContractStep {
            domain: "headset",
            name: "config_mapping_read",
            kind: StepKind::ConfigMappingRead,
            method: Methods::CONFIG_MAPPING,
            expected_params: json!({
                "cortexToken": token,
                "status": "read",
                "uuid": MAPPING_UUID,
            }),
            absent_params: vec!["name", "mappings"],
            response: json!({
                "message": "Read flex mapping config successful",
                "value": {
                    "label": {},
                    "mappings": config_mapping_create_mappings(),
                    "name": "Flex Contract",
                    "uuid": MAPPING_UUID
                }
            }),
        },
        ContractStep {
            domain: "headset",
            name: "config_mapping_update",
            kind: StepKind::ConfigMappingUpdate,
            method: Methods::CONFIG_MAPPING,
            expected_params: json!({
                "cortexToken": token,
                "status": "update",
                "uuid": MAPPING_UUID,
                "name": "Flex Contract Updated",
                "mappings": config_mapping_updated_mappings(),
            }),
            absent_params: vec![],
            response: json!({
                "message": "Update flex mapping config successful",
                "value": {
                    "label": {},
                    "mappings": config_mapping_updated_mappings(),
                    "name": "Flex Contract Updated",
                    "uuid": MAPPING_UUID
                }
            }),
        },
        ContractStep {
            domain: "headset",
            name: "config_mapping_delete",
            kind: StepKind::ConfigMappingDelete,
            method: Methods::CONFIG_MAPPING,
            expected_params: json!({
                "cortexToken": token,
                "status": "delete",
                "uuid": MAPPING_UUID,
            }),
            absent_params: vec!["name", "mappings"],
            response: json!({
                "message": "Delete flex mapping config successful",
                "uuid": MAPPING_UUID
            }),
        },
        ContractStep {
            domain: "headset",
            name: "update_headset",
            kind: StepKind::UpdateHeadset,
            method: Methods::UPDATE_HEADSET,
            expected_params: json!({
                "cortexToken": token,
                "headsetId": HEADSET_ID,
                "setting": {
                    "mode": "EPOCPLUS",
                    "eegRate": 256,
                    "memsRate": 64
                },
            }),
            absent_params: vec!["headset"],
            response: json!({
                "message": "Update headset successful",
                "headsetId": HEADSET_ID
            }),
        },
        ContractStep {
            domain: "headset",
            name: "update_headset_custom_info",
            kind: StepKind::UpdateHeadsetCustomInfo,
            method: Methods::UPDATE_HEADSET_CUSTOM_INFO,
            expected_params: json!({
                "cortexToken": token,
                "headsetId": HEADSET_ID,
                "headbandPosition": HEADBAND_POSITION,
                "customName": CUSTOM_HEADSET_NAME,
            }),
            absent_params: vec!["headset"],
            response: json!({
                "headsetId": HEADSET_ID,
                "headbandPosition": HEADBAND_POSITION,
                "customName": CUSTOM_HEADSET_NAME
            }),
        },
        ContractStep {
            domain: "headset",
            name: "sync_with_headset_clock",
            kind: StepKind::SyncWithHeadsetClock,
            method: Methods::SYNC_WITH_HEADSET_CLOCK,
            expected_params: json!({
                "headset": HEADSET_ID,
            }),
            absent_params: vec!["cortexToken", "headsetId"],
            response: json!({
                "adjustment": 0.0123,
                "headset": HEADSET_ID
            }),
        },
        ContractStep {
            domain: "session",
            name: "create_session",
            kind: StepKind::CreateSession,
            method: Methods::CREATE_SESSION,
            expected_params: json!({
                "cortexToken": token,
                "headset": HEADSET_ID,
                "status": "active",
            }),
            absent_params: vec![],
            response: session_response(),
        },
        ContractStep {
            domain: "session",
            name: "close_session",
            kind: StepKind::CloseSession,
            method: Methods::UPDATE_SESSION,
            expected_params: json!({
                "cortexToken": token,
                "session": SESSION_ID,
                "status": "close",
            }),
            absent_params: vec![],
            response: json!({
                "message": "Session closed"
            }),
        },
        ContractStep {
            domain: "session",
            name: "query_sessions",
            kind: StepKind::QuerySessions,
            method: Methods::QUERY_SESSIONS,
            expected_params: json!({
                "cortexToken": token,
            }),
            absent_params: vec![],
            response: json!([session_response()]),
        },
        ContractStep {
            domain: "subscription",
            name: "subscribe_streams",
            kind: StepKind::SubscribeStreams,
            method: Methods::SUBSCRIBE,
            expected_params: json!({
                "cortexToken": token,
                "session": SESSION_ID,
                "streams": CONTRACT_STREAMS,
            }),
            absent_params: vec![],
            response: json!({
                // Documented shape: success entries are objects with
                // streamName/cols/sid, not bare stream-name strings.
                "success": CONTRACT_STREAMS
                    .iter()
                    .map(|stream| json!({
                        "streamName": stream,
                        "cols": ["col1", "col2"],
                        "sid": SESSION_ID,
                    }))
                    .collect::<Vec<_>>(),
                "failure": []
            }),
        },
        ContractStep {
            domain: "subscription",
            name: "unsubscribe_streams",
            kind: StepKind::UnsubscribeStreams,
            method: Methods::UNSUBSCRIBE,
            expected_params: json!({
                "cortexToken": token,
                "session": SESSION_ID,
                "streams": CONTRACT_STREAMS,
            }),
            absent_params: vec![],
            response: json!({
                "success": CONTRACT_STREAMS
                    .iter()
                    .map(|stream| json!({
                        "streamName": stream,
                        "message": format!("The stream '{stream}' was successfully unsubscribed"),
                    }))
                    .collect::<Vec<_>>(),
                "failure": []
            }),
        },
        ContractStep {
            domain: "markers",
            name: "inject_marker",
            kind: StepKind::InjectMarker,
            method: Methods::INJECT_MARKER,
            expected_params: json!({
                "cortexToken": token,
                "session": SESSION_ID,
                "label": MARKER_LABEL,
                "value": 42,
                "port": "python-app",
                "time": 12345.0,
            }),
            absent_params: vec![],
            response: json!({
                "marker": {
                    "uuid": MARKER_ID,
                    "startDatetime": "2026-02-12T09:01:00Z"
                }
            }),
        },
        ContractStep {
            domain: "markers",
            name: "update_marker",
            kind: StepKind::UpdateMarker,
            method: Methods::UPDATE_MARKER,
            expected_params: json!({
                "cortexToken": token,
                "session": SESSION_ID,
                "markerId": MARKER_ID,
                "time": 12346.0,
            }),
            absent_params: vec![],
            response: json!({
                "message": "Marker updated"
            }),
        },
        ContractStep {
            domain: "records",
            name: "create_record",
            kind: StepKind::CreateRecord,
            method: Methods::CREATE_RECORD,
            expected_params: json!({
                "cortexToken": token,
                "session": SESSION_ID,
                "title": "Contract Record",
            }),
            absent_params: vec![],
            response: json!({
                "record": {
                    "uuid": "record-created",
                    "title": "Contract Record",
                    "startDatetime": "2026-02-12T09:00:00Z",
                }
            }),
        },
        ContractStep {
            domain: "records",
            name: "stop_record",
            kind: StepKind::StopRecord,
            method: Methods::STOP_RECORD,
            expected_params: json!({
                "cortexToken": token,
                "session": SESSION_ID,
            }),
            absent_params: vec![],
            response: json!({
                "record": {
                    "uuid": "record-created",
                    "title": "Contract Record",
                    "endDatetime": "2026-02-12T09:05:00Z",
                }
            }),
        },
        ContractStep {
            domain: "records",
            name: "query_records",
            kind: StepKind::QueryRecords,
            method: Methods::QUERY_RECORDS,
            expected_params: json!({
                "cortexToken": token,
                "query": {},
                "orderBy": [{ "startDatetime": "DESC" }],
                "limit": 10,
                "offset": 5,
            }),
            absent_params: vec![],
            response: json!({
                "records": [{
                    "uuid": "record-q1",
                    "title": "Recent Record",
                    "startDatetime": "2026-02-12T08:00:00Z"
                }]
            }),
        },
        ContractStep {
            domain: "records",
            name: "export_record",
            kind: StepKind::ExportRecord,
            method: Methods::EXPORT_RECORD,
            expected_params: json!({
                "cortexToken": token,
                "recordIds": record_ids(),
                "folder": "/tmp/export",
                "format": "CSV",
            }),
            absent_params: vec![],
            response: json!({
                "success": true
            }),
        },
        ContractStep {
            domain: "records",
            name: "update_record",
            kind: StepKind::UpdateRecord,
            method: Methods::UPDATE_RECORD,
            expected_params: json!({
                "cortexToken": token,
                "record": RECORD_ID,
                "title": "Updated Title",
                "description": "Updated Desc",
                "tags": record_tags(),
            }),
            absent_params: vec![],
            response: json!({
                "record": {
                    "uuid": RECORD_ID,
                    "title": "Updated Title",
                    "startDatetime": "2026-02-12T08:00:00Z"
                }
            }),
        },
        ContractStep {
            domain: "records",
            name: "delete_record",
            kind: StepKind::DeleteRecord,
            method: Methods::DELETE_RECORD,
            expected_params: json!({
                "cortexToken": token,
                "records": record_ids(),
            }),
            absent_params: vec![],
            response: json!({
                "deleted": 2
            }),
        },
        ContractStep {
            domain: "records",
            name: "get_record_infos",
            kind: StepKind::GetRecordInfos,
            method: Methods::GET_RECORD_INFOS,
            expected_params: json!({
                "cortexToken": token,
                "recordIds": record_ids(),
            }),
            absent_params: vec![],
            response: json!({
                "records": [{ "uuid": "record-1" }]
            }),
        },
        ContractStep {
            domain: "records",
            name: "config_opt_out",
            kind: StepKind::ConfigOptOut,
            method: Methods::CONFIG_OPT_OUT,
            expected_params: json!({
                "cortexToken": token,
                "status": "set",
                "newOptOut": true,
            }),
            absent_params: vec![],
            response: json!({
                "newOptOut": true
            }),
        },
        ContractStep {
            domain: "records",
            name: "download_record",
            kind: StepKind::DownloadRecord,
            method: Methods::DOWNLOAD_RECORD,
            expected_params: json!({
                "cortexToken": token,
                "recordIds": record_ids(),
            }),
            absent_params: vec![],
            response: json!({
                "requested": true
            }),
        },
        ContractStep {
            domain: "subjects",
            name: "create_subject",
            kind: StepKind::CreateSubject,
            method: Methods::CREATE_SUBJECT,
            expected_params: json!({
                "cortexToken": token,
                "subjectName": SUBJECT_NAME,
                "dateOfBirth": "1990-01-01",
                "sex": "F",
                "countryCode": "US",
                "state": "CA",
                "city": "San Francisco",
                "attributes": subject_attributes(),
            }),
            absent_params: vec![],
            response: json!({
                "subjectName": SUBJECT_NAME,
                "countryCode": "US",
                "city": "San Francisco",
            }),
        },
        ContractStep {
            domain: "subjects",
            name: "update_subject",
            kind: StepKind::UpdateSubject,
            method: Methods::UPDATE_SUBJECT,
            expected_params: json!({
                "cortexToken": token,
                "subjectName": SUBJECT_NAME,
                "city": "Los Angeles",
            }),
            absent_params: vec!["dateOfBirth", "sex", "countryCode", "state", "attributes"],
            response: json!({
                "subjectName": SUBJECT_NAME,
                "city": "Los Angeles",
            }),
        },
        ContractStep {
            domain: "subjects",
            name: "delete_subjects",
            kind: StepKind::DeleteSubjects,
            method: Methods::DELETE_SUBJECTS,
            expected_params: json!({
                "cortexToken": token,
                "subjects": subject_names(),
            }),
            absent_params: vec![],
            response: json!({
                "deleted": 2
            }),
        },
        ContractStep {
            domain: "subjects",
            name: "query_subjects",
            kind: StepKind::QuerySubjects,
            method: Methods::QUERY_SUBJECTS,
            expected_params: json!({
                "cortexToken": token,
                "query": subject_query(),
                "orderBy": subject_order(),
                "limit": 1,
                "offset": 0,
            }),
            absent_params: vec![],
            response: json!({
                "subjects": [{
                    "subjectName": SUBJECT_NAME,
                    "countryCode": "US"
                }],
                "count": 1
            }),
        },
        ContractStep {
            domain: "subjects",
            name: "get_demographic_attributes",
            kind: StepKind::GetDemographicAttributes,
            method: Methods::GET_DEMOGRAPHIC_ATTRIBUTES,
            expected_params: json!({
                "cortexToken": token,
            }),
            absent_params: vec![],
            response: json!([
                {
                    "name": "sex",
                    "value": ["M", "F", "U"]
                }
            ]),
        },
        ContractStep {
            domain: "profiles",
            name: "query_profiles",
            kind: StepKind::QueryProfiles,
            method: Methods::QUERY_PROFILE,
            expected_params: json!({
                "cortexToken": token,
            }),
            absent_params: vec![],
            response: json!([
                {
                    "uuid": "profile-1",
                    "name": PROFILE_NAME,
                    "readOnly": false,
                    "eegChannels": ["AF3", "AF4"]
                }
            ]),
        },
        ContractStep {
            domain: "profiles",
            name: "get_current_profile",
            kind: StepKind::GetCurrentProfile,
            method: Methods::GET_CURRENT_PROFILE,
            expected_params: json!({
                "cortexToken": token,
                "headset": HEADSET_ID,
            }),
            absent_params: vec![],
            response: json!({
                "name": PROFILE_NAME,
                "loadedByThisApp": true
            }),
        },
        ContractStep {
            domain: "profiles",
            name: "setup_profile",
            kind: StepKind::SetupProfile,
            method: Methods::SETUP_PROFILE,
            expected_params: json!({
                "cortexToken": token,
                "headset": HEADSET_ID,
                "profile": PROFILE_NAME,
                "status": "load",
            }),
            absent_params: vec![],
            response: json!({
                "success": true
            }),
        },
        ContractStep {
            domain: "profiles",
            name: "load_guest_profile",
            kind: StepKind::LoadGuestProfile,
            method: Methods::LOAD_GUEST_PROFILE,
            expected_params: json!({
                "cortexToken": token,
                "headset": HEADSET_ID,
            }),
            absent_params: vec![],
            response: json!({
                "success": true
            }),
        },
        ContractStep {
            domain: "advanced_bci",
            name: "get_detection_info",
            kind: StepKind::GetDetectionInfo,
            method: Methods::GET_DETECTION_INFO,
            expected_params: json!({
                "detection": "mentalCommand",
            }),
            absent_params: vec!["cortexToken"],
            response: json!({
                "actions": ["push", "pull"],
                "controls": ["start", "accept"],
                "events": ["MC_Succeeded"],
            }),
        },
        ContractStep {
            domain: "advanced_bci",
            name: "training",
            kind: StepKind::Training,
            method: Methods::TRAINING,
            expected_params: json!({
                "cortexToken": token,
                "session": SESSION_ID,
                "detection": "mentalCommand",
                "status": "start",
                "action": "push",
            }),
            absent_params: vec![],
            response: json!({
                "status": "ok"
            }),
        },
        ContractStep {
            domain: "advanced_bci",
            name: "mental_command_active_action",
            kind: StepKind::MentalCommandActiveAction,
            method: Methods::MENTAL_COMMAND_ACTIVE_ACTION,
            expected_params: json!({
                "cortexToken": token,
                "session": SESSION_ID,
                "status": "set",
                "actions": ["push", "pull"],
            }),
            absent_params: vec![],
            response: json!({
                "actions": ["push", "pull"]
            }),
        },
        ContractStep {
            domain: "advanced_bci",
            name: "mental_command_action_sensitivity",
            kind: StepKind::MentalCommandActionSensitivity,
            method: Methods::MENTAL_COMMAND_ACTION_SENSITIVITY,
            expected_params: json!({
                "cortexToken": token,
                "session": SESSION_ID,
                "status": "set",
                "values": [7, 6],
            }),
            absent_params: vec![],
            response: json!({
                "values": [7, 6]
            }),
        },
        ContractStep {
            domain: "advanced_bci",
            name: "mental_command_brain_map",
            kind: StepKind::MentalCommandBrainMap,
            method: Methods::MENTAL_COMMAND_BRAIN_MAP,
            expected_params: json!({
                "cortexToken": token,
                "session": SESSION_ID,
            }),
            absent_params: vec![],
            response: json!({
                "brainMap": []
            }),
        },
        ContractStep {
            domain: "advanced_bci",
            name: "mental_command_training_threshold",
            kind: StepKind::MentalCommandTrainingThreshold,
            method: Methods::MENTAL_COMMAND_TRAINING_THRESHOLD,
            expected_params: json!({
                "cortexToken": token,
                "session": SESSION_ID,
                "status": "get",
            }),
            absent_params: vec!["profile", "value"],
            response: json!({
                "value": 0.35
            }),
        },
        ContractStep {
            domain: "advanced_bci",
            name: "mental_command_training_threshold_for_profile",
            kind: StepKind::MentalCommandTrainingThresholdForProfile,
            method: Methods::MENTAL_COMMAND_TRAINING_THRESHOLD,
            expected_params: json!({
                "cortexToken": token,
                "profile": PROFILE_NAME,
                "status": "set",
                "value": 0.7,
            }),
            absent_params: vec!["session"],
            response: json!({
                "status": "set",
                "value": 0.7
            }),
        },
        ContractStep {
            domain: "advanced_bci",
            name: "mental_command_training_threshold_with_params",
            kind: StepKind::MentalCommandTrainingThresholdWithParams,
            method: Methods::MENTAL_COMMAND_TRAINING_THRESHOLD,
            expected_params: json!({
                "cortexToken": token,
                "session": SESSION_ID,
                "status": "set",
                "value": 0.9,
            }),
            absent_params: vec!["profile"],
            response: json!({
                "status": "set",
                "value": 0.9
            }),
        },
        ContractStep {
            domain: "advanced_bci",
            name: "get_trained_signature_actions",
            kind: StepKind::GetTrainedSignatureActions,
            method: Methods::GET_TRAINED_SIGNATURE_ACTIONS,
            expected_params: json!({
                "cortexToken": token,
                "detection": "mentalCommand",
                "profile": PROFILE_NAME,
            }),
            absent_params: vec!["session"],
            response: json!({
                "totalTimesTraining": 5,
                "trainedActions": [{
                    "action": "push",
                    "times": 5
                }]
            }),
        },
        ContractStep {
            domain: "advanced_bci",
            name: "get_training_time",
            kind: StepKind::GetTrainingTime,
            method: Methods::GET_TRAINING_TIME,
            expected_params: json!({
                "cortexToken": token,
                "detection": "mentalCommand",
                "session": SESSION_ID,
            }),
            absent_params: vec![],
            response: json!({
                "time": 9.5
            }),
        },
        ContractStep {
            domain: "advanced_bci",
            name: "facial_expression_signature_type",
            kind: StepKind::FacialExpressionSignatureType,
            method: Methods::FACIAL_EXPRESSION_SIGNATURE_TYPE,
            expected_params: json!({
                "cortexToken": token,
                "status": "set",
                "profile": PROFILE_NAME,
                "signature": "universal",
            }),
            absent_params: vec!["session"],
            response: json!({
                "signature": "universal"
            }),
        },
        ContractStep {
            domain: "advanced_bci",
            name: "facial_expression_threshold",
            kind: StepKind::FacialExpressionThreshold,
            method: Methods::FACIAL_EXPRESSION_THRESHOLD,
            expected_params: json!({
                "cortexToken": token,
                "status": "set",
                "action": FACIAL_ACTION,
                "profile": PROFILE_NAME,
                "value": 500,
            }),
            absent_params: vec!["session"],
            response: json!({
                "value": 500
            }),
        },
    ]
}
