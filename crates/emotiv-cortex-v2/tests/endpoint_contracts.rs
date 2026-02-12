mod support;

use emotiv_cortex_v2::protocol::{
    DetectionType, ExportFormat, Methods, ProfileAction, TrainingStatus,
};
use emotiv_cortex_v2::{CortexClient, CortexConfig, ResilientClient};
use serde_json::{json, Value};

use support::mock_cortex::MockCortexServer;

const TOKEN_CORTEX: &str = "token-contract";
const TOKEN_RESILIENT: &str = "token-resilient";
const SESSION_ID: &str = "session-001";
const HEADSET_ID: &str = "HS-001";
const PROFILE_NAME: &str = "profile-alpha";
const RECORD_ID: &str = "record-1";
const SUBJECT_NAME: &str = "subject-a";
const FACIAL_ACTION: &str = "smile";

#[derive(Debug, Clone)]
enum StepKind {
    // Records
    CreateRecord,
    StopRecord,
    QueryRecords,
    ExportRecord,
    UpdateRecord,
    DeleteRecord,
    GetRecordInfos,
    ConfigOptOut,
    DownloadRecord,
    // Subjects
    CreateSubject,
    UpdateSubject,
    DeleteSubjects,
    QuerySubjects,
    GetDemographicAttributes,
    // Profiles
    QueryProfiles,
    GetCurrentProfile,
    SetupProfile,
    LoadGuestProfile,
    // Advanced BCI
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
struct ContractStep {
    domain: &'static str,
    name: &'static str,
    kind: StepKind,
    method: &'static str,
    expected_params: Value,
    absent_params: Vec<&'static str>,
    response: Value,
}

fn record_ids() -> Vec<String> {
    vec!["record-1".to_string(), "record-2".to_string()]
}

fn record_tags() -> Vec<String> {
    vec!["tag-a".to_string(), "tag-b".to_string()]
}

fn subject_names() -> Vec<String> {
    vec!["subject-a".to_string(), "subject-b".to_string()]
}

fn subject_attributes() -> Vec<Value> {
    vec![json!({
        "name": "handedness",
        "value": "right",
    })]
}

fn subject_query() -> Value {
    json!({
        "subjectName": SUBJECT_NAME,
    })
}

fn subject_order() -> Value {
    json!([{
        "subjectName": "ASC",
    }])
}

fn build_contract_steps(token: &str) -> Vec<ContractStep> {
    vec![
        // --- Records ---
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
                "records": [{
                    "uuid": "record-1"
                }]
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
        // --- Subjects ---
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
            absent_params: vec![
                "dateOfBirth",
                "sex",
                "countryCode",
                "state",
                "attributes",
            ],
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
        // --- Profiles ---
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
        // --- Advanced BCI ---
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

fn rpc_id(request: &Value) -> u64 {
    request
        .get("id")
        .and_then(Value::as_u64)
        .expect("request missing numeric id")
}

fn assert_request_matches_step(request: &Value, step: &ContractStep) {
    let method = request.get("method").and_then(Value::as_str);
    assert_eq!(
        method,
        Some(step.method),
        "unexpected method for {}::{}",
        step.domain,
        step.name
    );

    let params = request
        .get("params")
        .and_then(Value::as_object)
        .expect("request missing params object");

    let expected = step
        .expected_params
        .as_object()
        .expect("expected_params must be an object");

    for (key, expected_value) in expected {
        assert_eq!(
            params.get(key),
            Some(expected_value),
            "mismatch in {}::{} param '{}'",
            step.domain,
            step.name,
            key
        );
    }

    for key in &step.absent_params {
        assert!(
            !params.contains_key(*key),
            "{}::{} unexpectedly included param '{}'",
            step.domain,
            step.name,
            key
        );
    }
}

async fn execute_cortex_step(client: &CortexClient, kind: &StepKind) {
    match kind {
        StepKind::CreateRecord => {
            let record = client
                .create_record(TOKEN_CORTEX, SESSION_ID, "Contract Record")
                .await
                .unwrap();
            assert_eq!(record.uuid, "record-created");
        }
        StepKind::StopRecord => {
            let record = client.stop_record(TOKEN_CORTEX, SESSION_ID).await.unwrap();
            assert_eq!(record.uuid, "record-created");
        }
        StepKind::QueryRecords => {
            let records = client.query_records(TOKEN_CORTEX, Some(10), Some(5)).await.unwrap();
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].uuid, "record-q1");
        }
        StepKind::ExportRecord => {
            client
                .export_record(TOKEN_CORTEX, &record_ids(), "/tmp/export", ExportFormat::Csv)
                .await
                .unwrap();
        }
        StepKind::UpdateRecord => {
            let record = client
                .update_record(
                    TOKEN_CORTEX,
                    RECORD_ID,
                    Some("Updated Title"),
                    Some("Updated Desc"),
                    Some(&record_tags()),
                )
                .await
                .unwrap();
            assert_eq!(record.uuid, RECORD_ID);
            assert_eq!(record.title.as_deref(), Some("Updated Title"));
        }
        StepKind::DeleteRecord => {
            let result = client.delete_record(TOKEN_CORTEX, &record_ids()).await.unwrap();
            assert_eq!(result["deleted"], 2);
        }
        StepKind::GetRecordInfos => {
            let result = client
                .get_record_infos(TOKEN_CORTEX, &record_ids())
                .await
                .unwrap();
            assert_eq!(result["records"][0]["uuid"], "record-1");
        }
        StepKind::ConfigOptOut => {
            let result = client.config_opt_out(TOKEN_CORTEX, "set", Some(true)).await.unwrap();
            assert_eq!(result["newOptOut"], true);
        }
        StepKind::DownloadRecord => {
            let result = client
                .download_record(TOKEN_CORTEX, &record_ids())
                .await
                .unwrap();
            assert_eq!(result["requested"], true);
        }
        StepKind::CreateSubject => {
            let subject = client
                .create_subject(
                    TOKEN_CORTEX,
                    SUBJECT_NAME,
                    Some("1990-01-01"),
                    Some("F"),
                    Some("US"),
                    Some("CA"),
                    Some("San Francisco"),
                    Some(&subject_attributes()),
                )
                .await
                .unwrap();
            assert_eq!(subject.subject_name, SUBJECT_NAME);
            assert_eq!(subject.country_code.as_deref(), Some("US"));
        }
        StepKind::UpdateSubject => {
            let subject = client
                .update_subject(
                    TOKEN_CORTEX,
                    SUBJECT_NAME,
                    None,
                    None,
                    None,
                    None,
                    Some("Los Angeles"),
                    None,
                )
                .await
                .unwrap();
            assert_eq!(subject.subject_name, SUBJECT_NAME);
            assert_eq!(subject.city.as_deref(), Some("Los Angeles"));
        }
        StepKind::DeleteSubjects => {
            let result = client
                .delete_subjects(TOKEN_CORTEX, &subject_names())
                .await
                .unwrap();
            assert_eq!(result["deleted"], 2);
        }
        StepKind::QuerySubjects => {
            let (subjects, count) = client
                .query_subjects(
                    TOKEN_CORTEX,
                    subject_query(),
                    subject_order(),
                    Some(1),
                    Some(0),
                )
                .await
                .unwrap();
            assert_eq!(count, 1);
            assert_eq!(subjects.len(), 1);
            assert_eq!(subjects[0].subject_name, SUBJECT_NAME);
        }
        StepKind::GetDemographicAttributes => {
            let attrs = client
                .get_demographic_attributes(TOKEN_CORTEX)
                .await
                .unwrap();
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0].name, "sex");
        }
        StepKind::QueryProfiles => {
            let profiles = client.query_profiles(TOKEN_CORTEX).await.unwrap();
            assert_eq!(profiles.len(), 1);
            assert_eq!(profiles[0].name, PROFILE_NAME);
        }
        StepKind::GetCurrentProfile => {
            let profile = client
                .get_current_profile(TOKEN_CORTEX, HEADSET_ID)
                .await
                .unwrap();
            assert_eq!(profile.name.as_deref(), Some(PROFILE_NAME));
            assert!(profile.loaded_by_this_app);
        }
        StepKind::SetupProfile => {
            client
                .setup_profile(TOKEN_CORTEX, HEADSET_ID, PROFILE_NAME, ProfileAction::Load)
                .await
                .unwrap();
        }
        StepKind::LoadGuestProfile => {
            client
                .load_guest_profile(TOKEN_CORTEX, HEADSET_ID)
                .await
                .unwrap();
        }
        StepKind::GetDetectionInfo => {
            let info = client
                .get_detection_info(DetectionType::MentalCommand)
                .await
                .unwrap();
            assert_eq!(info.actions, vec!["push".to_string(), "pull".to_string()]);
        }
        StepKind::Training => {
            let result = client
                .training(
                    TOKEN_CORTEX,
                    SESSION_ID,
                    DetectionType::MentalCommand,
                    TrainingStatus::Start,
                    "push",
                )
                .await
                .unwrap();
            assert_eq!(result["status"], "ok");
        }
        StepKind::MentalCommandActiveAction => {
            let actions = ["push", "pull"];
            let result = client
                .mental_command_active_action(TOKEN_CORTEX, SESSION_ID, Some(&actions))
                .await
                .unwrap();
            assert_eq!(result["actions"][0], "push");
        }
        StepKind::MentalCommandActionSensitivity => {
            let values = [7, 6];
            let result = client
                .mental_command_action_sensitivity(TOKEN_CORTEX, SESSION_ID, Some(&values))
                .await
                .unwrap();
            assert_eq!(result["values"][1], 6);
        }
        StepKind::MentalCommandBrainMap => {
            let result = client
                .mental_command_brain_map(TOKEN_CORTEX, SESSION_ID)
                .await
                .unwrap();
            assert!(result["brainMap"].is_array());
        }
        StepKind::MentalCommandTrainingThreshold => {
            let result = client
                .mental_command_training_threshold(TOKEN_CORTEX, SESSION_ID)
                .await
                .unwrap();
            assert_eq!(result["value"], 0.35);
        }
        StepKind::MentalCommandTrainingThresholdForProfile => {
            let result = client
                .mental_command_training_threshold_for_profile(
                    TOKEN_CORTEX,
                    PROFILE_NAME,
                    Some("set"),
                    Some(0.7),
                )
                .await
                .unwrap();
            assert_eq!(result["status"], "set");
        }
        StepKind::MentalCommandTrainingThresholdWithParams => {
            let result = client
                .mental_command_training_threshold_with_params(
                    TOKEN_CORTEX,
                    Some(SESSION_ID),
                    None,
                    Some("set"),
                    Some(0.9),
                )
                .await
                .unwrap();
            assert_eq!(result["value"], 0.9);
        }
        StepKind::GetTrainedSignatureActions => {
            let actions = client
                .get_trained_signature_actions(
                    TOKEN_CORTEX,
                    DetectionType::MentalCommand,
                    Some(PROFILE_NAME),
                    None,
                )
                .await
                .unwrap();
            assert_eq!(actions.total_times_training, 5);
            assert_eq!(actions.trained_actions[0].action, "push");
        }
        StepKind::GetTrainingTime => {
            let time = client
                .get_training_time(TOKEN_CORTEX, DetectionType::MentalCommand, SESSION_ID)
                .await
                .unwrap();
            assert!((time.time - 9.5).abs() < f64::EPSILON);
        }
        StepKind::FacialExpressionSignatureType => {
            let result = client
                .facial_expression_signature_type(
                    TOKEN_CORTEX,
                    "set",
                    Some(PROFILE_NAME),
                    None,
                    Some("universal"),
                )
                .await
                .unwrap();
            assert_eq!(result["signature"], "universal");
        }
        StepKind::FacialExpressionThreshold => {
            let result = client
                .facial_expression_threshold(
                    TOKEN_CORTEX,
                    "set",
                    FACIAL_ACTION,
                    Some(PROFILE_NAME),
                    None,
                    Some(500),
                )
                .await
                .unwrap();
            assert_eq!(result["value"], 500);
        }
    }
}

async fn execute_resilient_step(client: &ResilientClient, kind: &StepKind) {
    match kind {
        StepKind::CreateRecord => {
            let record = client
                .create_record(SESSION_ID, "Contract Record")
                .await
                .unwrap();
            assert_eq!(record.uuid, "record-created");
        }
        StepKind::StopRecord => {
            let record = client.stop_record(SESSION_ID).await.unwrap();
            assert_eq!(record.uuid, "record-created");
        }
        StepKind::QueryRecords => {
            let records = client.query_records(Some(10), Some(5)).await.unwrap();
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].uuid, "record-q1");
        }
        StepKind::ExportRecord => {
            client
                .export_record(&record_ids(), "/tmp/export", ExportFormat::Csv)
                .await
                .unwrap();
        }
        StepKind::UpdateRecord => {
            let record = client
                .update_record(
                    RECORD_ID,
                    Some("Updated Title"),
                    Some("Updated Desc"),
                    Some(&record_tags()),
                )
                .await
                .unwrap();
            assert_eq!(record.uuid, RECORD_ID);
            assert_eq!(record.title.as_deref(), Some("Updated Title"));
        }
        StepKind::DeleteRecord => {
            let result = client.delete_record(&record_ids()).await.unwrap();
            assert_eq!(result["deleted"], 2);
        }
        StepKind::GetRecordInfos => {
            let result = client.get_record_infos(&record_ids()).await.unwrap();
            assert_eq!(result["records"][0]["uuid"], "record-1");
        }
        StepKind::ConfigOptOut => {
            let result = client.config_opt_out("set", Some(true)).await.unwrap();
            assert_eq!(result["newOptOut"], true);
        }
        StepKind::DownloadRecord => {
            let result = client.download_record(&record_ids()).await.unwrap();
            assert_eq!(result["requested"], true);
        }
        StepKind::CreateSubject => {
            let subject = client
                .create_subject(
                    SUBJECT_NAME,
                    Some("1990-01-01"),
                    Some("F"),
                    Some("US"),
                    Some("CA"),
                    Some("San Francisco"),
                    Some(&subject_attributes()),
                )
                .await
                .unwrap();
            assert_eq!(subject.subject_name, SUBJECT_NAME);
            assert_eq!(subject.country_code.as_deref(), Some("US"));
        }
        StepKind::UpdateSubject => {
            let subject = client
                .update_subject(
                    SUBJECT_NAME,
                    None,
                    None,
                    None,
                    None,
                    Some("Los Angeles"),
                    None,
                )
                .await
                .unwrap();
            assert_eq!(subject.subject_name, SUBJECT_NAME);
            assert_eq!(subject.city.as_deref(), Some("Los Angeles"));
        }
        StepKind::DeleteSubjects => {
            let result = client.delete_subjects(&subject_names()).await.unwrap();
            assert_eq!(result["deleted"], 2);
        }
        StepKind::QuerySubjects => {
            let (subjects, count) = client
                .query_subjects(subject_query(), subject_order(), Some(1), Some(0))
                .await
                .unwrap();
            assert_eq!(count, 1);
            assert_eq!(subjects.len(), 1);
            assert_eq!(subjects[0].subject_name, SUBJECT_NAME);
        }
        StepKind::GetDemographicAttributes => {
            let attrs = client.get_demographic_attributes().await.unwrap();
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0].name, "sex");
        }
        StepKind::QueryProfiles => {
            let profiles = client.query_profiles().await.unwrap();
            assert_eq!(profiles.len(), 1);
            assert_eq!(profiles[0].name, PROFILE_NAME);
        }
        StepKind::GetCurrentProfile => {
            let profile = client.get_current_profile(HEADSET_ID).await.unwrap();
            assert_eq!(profile.name.as_deref(), Some(PROFILE_NAME));
            assert!(profile.loaded_by_this_app);
        }
        StepKind::SetupProfile => {
            client
                .setup_profile(HEADSET_ID, PROFILE_NAME, ProfileAction::Load)
                .await
                .unwrap();
        }
        StepKind::LoadGuestProfile => {
            client.load_guest_profile(HEADSET_ID).await.unwrap();
        }
        StepKind::GetDetectionInfo => {
            let info = client
                .get_detection_info(DetectionType::MentalCommand)
                .await
                .unwrap();
            assert_eq!(info.actions, vec!["push".to_string(), "pull".to_string()]);
        }
        StepKind::Training => {
            let result = client
                .training(
                    SESSION_ID,
                    DetectionType::MentalCommand,
                    TrainingStatus::Start,
                    "push",
                )
                .await
                .unwrap();
            assert_eq!(result["status"], "ok");
        }
        StepKind::MentalCommandActiveAction => {
            let actions = ["push", "pull"];
            let result = client
                .mental_command_active_action(SESSION_ID, Some(&actions))
                .await
                .unwrap();
            assert_eq!(result["actions"][0], "push");
        }
        StepKind::MentalCommandActionSensitivity => {
            let values = [7, 6];
            let result = client
                .mental_command_action_sensitivity(SESSION_ID, Some(&values))
                .await
                .unwrap();
            assert_eq!(result["values"][1], 6);
        }
        StepKind::MentalCommandBrainMap => {
            let result = client.mental_command_brain_map(SESSION_ID).await.unwrap();
            assert!(result["brainMap"].is_array());
        }
        StepKind::MentalCommandTrainingThreshold => {
            let result = client
                .mental_command_training_threshold(SESSION_ID)
                .await
                .unwrap();
            assert_eq!(result["value"], 0.35);
        }
        StepKind::MentalCommandTrainingThresholdForProfile => {
            let result = client
                .mental_command_training_threshold_for_profile(
                    PROFILE_NAME,
                    Some("set"),
                    Some(0.7),
                )
                .await
                .unwrap();
            assert_eq!(result["status"], "set");
        }
        StepKind::MentalCommandTrainingThresholdWithParams => {
            let result = client
                .mental_command_training_threshold_with_params(
                    Some(SESSION_ID),
                    None,
                    Some("set"),
                    Some(0.9),
                )
                .await
                .unwrap();
            assert_eq!(result["value"], 0.9);
        }
        StepKind::GetTrainedSignatureActions => {
            let actions = client
                .get_trained_signature_actions(
                    DetectionType::MentalCommand,
                    Some(PROFILE_NAME),
                    None,
                )
                .await
                .unwrap();
            assert_eq!(actions.total_times_training, 5);
            assert_eq!(actions.trained_actions[0].action, "push");
        }
        StepKind::GetTrainingTime => {
            let time = client
                .get_training_time(DetectionType::MentalCommand, SESSION_ID)
                .await
                .unwrap();
            assert!((time.time - 9.5).abs() < f64::EPSILON);
        }
        StepKind::FacialExpressionSignatureType => {
            let result = client
                .facial_expression_signature_type(
                    "set",
                    Some(PROFILE_NAME),
                    None,
                    Some("universal"),
                )
                .await
                .unwrap();
            assert_eq!(result["signature"], "universal");
        }
        StepKind::FacialExpressionThreshold => {
            let result = client
                .facial_expression_threshold(
                    "set",
                    FACIAL_ACTION,
                    Some(PROFILE_NAME),
                    None,
                    Some(500),
                )
                .await
                .unwrap();
            assert_eq!(result["value"], 500);
        }
    }
}

fn cortex_contract_config(url: String) -> CortexConfig {
    let mut config = CortexConfig::new("test-client-id", "test-client-secret");
    config.cortex_url = url;
    config.timeouts.rpc_timeout_secs = 2;
    config
}

fn resilient_contract_config(url: String) -> CortexConfig {
    let mut config = CortexConfig::new("test-client-id", "test-client-secret");
    config.cortex_url = url;
    config.reconnect.enabled = false;
    config.health.enabled = false;
    config.timeouts.rpc_timeout_secs = 2;
    config
}

async fn start_server_or_skip(test_name: &str) -> Option<MockCortexServer> {
    match MockCortexServer::start().await {
        Ok(server) => Some(server),
        Err(err) => {
            eprintln!("Skipping {test_name}: unable to start mock server: {err}");
            None
        }
    }
}

async fn drive_auth_handshake(
    connection: &mut support::mock_cortex::MockConnection,
    token: &str,
) {
    let info = connection.recv_request_method(Methods::GET_CORTEX_INFO).await;
    connection.send_result(rpc_id(&info), json!({"version": "mock"})).await;

    let request_access = connection.recv_request_method(Methods::REQUEST_ACCESS).await;
    connection
        .send_result(rpc_id(&request_access), json!({"accessGranted": true}))
        .await;

    let authorize = connection.recv_request_method(Methods::AUTHORIZE).await;
    connection
        .send_result(rpc_id(&authorize), json!({"cortexToken": token}))
        .await;
}

#[tokio::test]
async fn cortex_client_endpoint_contracts_table_driven() {
    let mut server = match start_server_or_skip("cortex_client_endpoint_contracts_table_driven").await
    {
        Some(server) => server,
        None => return,
    };

    let steps = build_contract_steps(TOKEN_CORTEX);
    let server_steps = steps.clone();
    let url = server.ws_url();
    let server_task = tokio::spawn(async move {
        let mut connection = server.accept_connection().await;
        for step in server_steps {
            let request = connection.recv_request().await;
            assert_request_matches_step(&request, &step);
            connection
                .send_result(rpc_id(&request), step.response.clone())
                .await;
        }
    });

    let config = cortex_contract_config(url);
    let mut client = CortexClient::connect(&config).await.unwrap();

    for step in &steps {
        execute_cortex_step(&client, &step.kind).await;
    }

    server_task.await.unwrap();
    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn resilient_client_endpoint_contracts_table_driven() {
    let mut server =
        match start_server_or_skip("resilient_client_endpoint_contracts_table_driven").await {
            Some(server) => server,
            None => return,
        };

    let steps = build_contract_steps(TOKEN_RESILIENT);
    let server_steps = steps.clone();
    let url = server.ws_url();
    let server_task = tokio::spawn(async move {
        let mut connection = server.accept_connection().await;
        drive_auth_handshake(&mut connection, TOKEN_RESILIENT).await;

        for step in server_steps {
            let request = connection.recv_request().await;
            assert_request_matches_step(&request, &step);
            connection
                .send_result(rpc_id(&request), step.response.clone())
                .await;
        }
    });

    let config = resilient_contract_config(url);
    let client = ResilientClient::connect(config).await.unwrap();
    assert_eq!(client.cortex_token().await, TOKEN_RESILIENT);

    for step in &steps {
        execute_resilient_step(&client, &step.kind).await;
    }

    client.disconnect().await.unwrap();
    server_task.await.unwrap();
}
