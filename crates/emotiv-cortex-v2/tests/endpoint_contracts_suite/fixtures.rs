use emotiv_cortex_v2::protocol::constants::Methods;
use serde_json::{Value, json};

pub(super) const TOKEN_CORTEX: &str = "token-contract";
pub(super) const TOKEN_RESILIENT: &str = "token-resilient";
pub(super) const SESSION_ID: &str = "session-001";
pub(super) const HEADSET_ID: &str = "HS-001";
pub(super) const PROFILE_NAME: &str = "profile-alpha";
pub(super) const RECORD_ID: &str = "record-1";
pub(super) const SUBJECT_NAME: &str = "subject-a";
pub(super) const FACIAL_ACTION: &str = "smile";

#[derive(Debug, Clone)]
pub(super) enum StepKind {
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

pub(super) fn build_contract_steps(token: &str) -> Vec<ContractStep> {
    vec![
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
