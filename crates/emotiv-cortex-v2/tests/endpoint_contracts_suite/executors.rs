use emotiv_cortex_v2::protocol::profiles::ProfileAction;
use emotiv_cortex_v2::protocol::records::{ExportFormat, UpdateRecordRequest};
use emotiv_cortex_v2::protocol::subjects::{QuerySubjectsRequest, SubjectRequest};
use emotiv_cortex_v2::protocol::training::{
    DetectionType, FacialExpressionSignatureTypeRequest, FacialExpressionThresholdRequest,
    MentalCommandTrainingThresholdRequest, TrainingStatus,
};
use emotiv_cortex_v2::{CortexClient, ResilientClient};

use super::fixtures::{
    FACIAL_ACTION, HEADSET_ID, PROFILE_NAME, RECORD_ID, SESSION_ID, SUBJECT_NAME, StepKind,
    TOKEN_CORTEX, record_ids, record_tags, subject_attributes, subject_names, subject_order,
    subject_query,
};

pub(super) async fn execute_cortex_step(client: &CortexClient, kind: &StepKind) {
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
            let records = client
                .query_records(TOKEN_CORTEX, Some(10), Some(5))
                .await
                .unwrap();
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].uuid, "record-q1");
        }
        StepKind::ExportRecord => {
            client
                .export_record(
                    TOKEN_CORTEX,
                    &record_ids(),
                    "/tmp/export",
                    ExportFormat::Csv,
                )
                .await
                .unwrap();
        }
        StepKind::UpdateRecord => {
            let request = UpdateRecordRequest {
                record_id: RECORD_ID.to_string(),
                title: Some("Updated Title".to_string()),
                description: Some("Updated Desc".to_string()),
                tags: Some(record_tags()),
            };
            let record = client
                .update_record_with(TOKEN_CORTEX, &request)
                .await
                .unwrap();
            assert_eq!(record.uuid, RECORD_ID);
            assert_eq!(record.title.as_deref(), Some("Updated Title"));
        }
        StepKind::DeleteRecord => {
            let result = client
                .delete_record(TOKEN_CORTEX, &record_ids())
                .await
                .unwrap();
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
            let result = client
                .config_opt_out(TOKEN_CORTEX, "set", Some(true))
                .await
                .unwrap();
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
            let request = SubjectRequest {
                subject_name: SUBJECT_NAME.to_string(),
                date_of_birth: Some("1990-01-01".to_string()),
                sex: Some("F".to_string()),
                country_code: Some("US".to_string()),
                state: Some("CA".to_string()),
                city: Some("San Francisco".to_string()),
                attributes: Some(subject_attributes()),
            };
            let subject = client
                .create_subject_with(TOKEN_CORTEX, &request)
                .await
                .unwrap();
            assert_eq!(subject.subject_name, SUBJECT_NAME);
            assert_eq!(subject.country_code.as_deref(), Some("US"));
        }
        StepKind::UpdateSubject => {
            let request = SubjectRequest {
                subject_name: SUBJECT_NAME.to_string(),
                date_of_birth: None,
                sex: None,
                country_code: None,
                state: None,
                city: Some("Los Angeles".to_string()),
                attributes: None,
            };
            let subject = client
                .update_subject_with(TOKEN_CORTEX, &request)
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
            let request = QuerySubjectsRequest {
                query: subject_query(),
                order_by: subject_order(),
                limit: Some(1),
                offset: Some(0),
            };
            let (subjects, count) = client
                .query_subjects_with(TOKEN_CORTEX, &request)
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
            let request = MentalCommandTrainingThresholdRequest {
                session_id: None,
                profile: Some(PROFILE_NAME.to_string()),
                status: Some("set".to_string()),
                value: Some(0.7),
            };
            let result = client
                .mental_command_training_threshold_with_request(TOKEN_CORTEX, &request)
                .await
                .unwrap();
            assert_eq!(result["status"], "set");
        }
        StepKind::MentalCommandTrainingThresholdWithParams => {
            let request = MentalCommandTrainingThresholdRequest {
                session_id: Some(SESSION_ID.to_string()),
                profile: None,
                status: Some("set".to_string()),
                value: Some(0.9),
            };
            let result = client
                .mental_command_training_threshold_with_request(TOKEN_CORTEX, &request)
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
            let request = FacialExpressionSignatureTypeRequest {
                status: "set".to_string(),
                profile: Some(PROFILE_NAME.to_string()),
                session: None,
                signature: Some("universal".to_string()),
            };
            let result = client
                .facial_expression_signature_type_with(TOKEN_CORTEX, &request)
                .await
                .unwrap();
            assert_eq!(result["signature"], "universal");
        }
        StepKind::FacialExpressionThreshold => {
            let request = FacialExpressionThresholdRequest {
                status: "set".to_string(),
                action: FACIAL_ACTION.to_string(),
                profile: Some(PROFILE_NAME.to_string()),
                session: None,
                value: Some(500),
            };
            let result = client
                .facial_expression_threshold_with(TOKEN_CORTEX, &request)
                .await
                .unwrap();
            assert_eq!(result["value"], 500);
        }
    }
}

pub(super) async fn execute_resilient_step(client: &ResilientClient, kind: &StepKind) {
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
            let request = UpdateRecordRequest {
                record_id: RECORD_ID.to_string(),
                title: Some("Updated Title".to_string()),
                description: Some("Updated Desc".to_string()),
                tags: Some(record_tags()),
            };
            let record = client.update_record_with(&request).await.unwrap();
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
            let request = SubjectRequest {
                subject_name: SUBJECT_NAME.to_string(),
                date_of_birth: Some("1990-01-01".to_string()),
                sex: Some("F".to_string()),
                country_code: Some("US".to_string()),
                state: Some("CA".to_string()),
                city: Some("San Francisco".to_string()),
                attributes: Some(subject_attributes()),
            };
            let subject = client.create_subject_with(&request).await.unwrap();
            assert_eq!(subject.subject_name, SUBJECT_NAME);
            assert_eq!(subject.country_code.as_deref(), Some("US"));
        }
        StepKind::UpdateSubject => {
            let request = SubjectRequest {
                subject_name: SUBJECT_NAME.to_string(),
                date_of_birth: None,
                sex: None,
                country_code: None,
                state: None,
                city: Some("Los Angeles".to_string()),
                attributes: None,
            };
            let subject = client.update_subject_with(&request).await.unwrap();
            assert_eq!(subject.subject_name, SUBJECT_NAME);
            assert_eq!(subject.city.as_deref(), Some("Los Angeles"));
        }
        StepKind::DeleteSubjects => {
            let result = client.delete_subjects(&subject_names()).await.unwrap();
            assert_eq!(result["deleted"], 2);
        }
        StepKind::QuerySubjects => {
            let request = QuerySubjectsRequest {
                query: subject_query(),
                order_by: subject_order(),
                limit: Some(1),
                offset: Some(0),
            };
            let (subjects, count) = client.query_subjects_with(&request).await.unwrap();
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
            let request = MentalCommandTrainingThresholdRequest {
                session_id: None,
                profile: Some(PROFILE_NAME.to_string()),
                status: Some("set".to_string()),
                value: Some(0.7),
            };
            let result = client
                .mental_command_training_threshold_with_request(&request)
                .await
                .unwrap();
            assert_eq!(result["status"], "set");
        }
        StepKind::MentalCommandTrainingThresholdWithParams => {
            let request = MentalCommandTrainingThresholdRequest {
                session_id: Some(SESSION_ID.to_string()),
                profile: None,
                status: Some("set".to_string()),
                value: Some(0.9),
            };
            let result = client
                .mental_command_training_threshold_with_request(&request)
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
            let request = FacialExpressionSignatureTypeRequest {
                status: "set".to_string(),
                profile: Some(PROFILE_NAME.to_string()),
                session: None,
                signature: Some("universal".to_string()),
            };
            let result = client
                .facial_expression_signature_type_with(&request)
                .await
                .unwrap();
            assert_eq!(result["signature"], "universal");
        }
        StepKind::FacialExpressionThreshold => {
            let request = FacialExpressionThresholdRequest {
                status: "set".to_string(),
                action: FACIAL_ACTION.to_string(),
                profile: Some(PROFILE_NAME.to_string()),
                session: None,
                value: Some(500),
            };
            let result = client
                .facial_expression_threshold_with(&request)
                .await
                .unwrap();
            assert_eq!(result["value"], 500);
        }
    }
}
