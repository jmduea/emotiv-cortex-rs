use emotiv_cortex_v2::protocol::headset::{ConfigMappingRequest, ConfigMappingResponse};
use emotiv_cortex_v2::protocol::profiles::ProfileAction;
use emotiv_cortex_v2::protocol::records::{ExportFormat, UpdateRecordRequest};
use emotiv_cortex_v2::protocol::subjects::{QuerySubjectsRequest, SubjectRequest};
use emotiv_cortex_v2::protocol::training::{
    DetectionType, FacialExpressionSignatureTypeRequest, FacialExpressionThresholdRequest,
    MentalCommandTrainingThresholdRequest, TrainingStatus,
};
use emotiv_cortex_v2::{CortexClient, ResilientClient};

use super::fixtures::{
    CLIENT_ID, CLIENT_SECRET, CONTRACT_STREAMS, CUSTOM_HEADSET_NAME, FACIAL_ACTION,
    HEADBAND_POSITION, HEADSET_ID, MAPPING_UUID, MARKER_ID, MARKER_LABEL, PROFILE_NAME, RECORD_ID,
    SESSION_ID, SUBJECT_NAME, StepKind, TOKEN_CORTEX, config_mapping_create_mappings,
    config_mapping_updated_mappings, record_ids, record_tags, subject_attributes, subject_names,
    subject_order, subject_query,
};

pub(super) async fn execute_cortex_step(client: &CortexClient, kind: &StepKind) {
    match kind {
        StepKind::HasAccessRight => {
            let granted = client
                .has_access_right(CLIENT_ID, CLIENT_SECRET)
                .await
                .unwrap();
            assert!(granted);
        }
        StepKind::GetUserLogin => {
            let users = client.get_user_login().await.unwrap();
            assert_eq!(users.len(), 1);
            assert_eq!(users[0].username, "contract-user");
            assert_eq!(users[0].logged_in_os_uid.as_deref(), Some("launcher"));
        }
        StepKind::GetUserInfo => {
            let result = client.get_user_info(TOKEN_CORTEX).await.unwrap();
            assert_eq!(result["username"], "contract-user");
            assert_eq!(result["firstName"], "Contract");
        }
        StepKind::GetLicenseInfo => {
            let result = client.get_license_info(TOKEN_CORTEX).await.unwrap();
            assert_eq!(result["isOnline"], true);
            assert_eq!(result["license"]["id"], "license-001");
        }
        StepKind::ConnectHeadset => {
            client.connect_headset(HEADSET_ID).await.unwrap();
        }
        StepKind::DisconnectHeadset => {
            client.disconnect_headset(HEADSET_ID).await.unwrap();
        }
        StepKind::RefreshHeadsets => {
            client.refresh_headsets().await.unwrap();
        }
        StepKind::ConfigMappingCreate => {
            let response = client
                .config_mapping(
                    TOKEN_CORTEX,
                    ConfigMappingRequest::Create {
                        name: "Flex Contract".to_string(),
                        mappings: config_mapping_create_mappings(),
                    },
                )
                .await
                .unwrap();
            match response {
                ConfigMappingResponse::Value { message, value } => {
                    assert_eq!(message, "Create flex mapping config successful");
                    assert_eq!(value.uuid, MAPPING_UUID);
                }
                other => panic!("unexpected config mapping create response: {other:?}"),
            }
        }
        StepKind::ConfigMappingGet => {
            let response = client
                .config_mapping(TOKEN_CORTEX, ConfigMappingRequest::Get)
                .await
                .unwrap();
            match response {
                ConfigMappingResponse::List { message, value } => {
                    assert_eq!(message, "Get flex mapping config successful");
                    assert_eq!(value.config.len(), 1);
                    assert_eq!(value.config[0].uuid, MAPPING_UUID);
                }
                other => panic!("unexpected config mapping get response: {other:?}"),
            }
        }
        StepKind::ConfigMappingRead => {
            let response = client
                .config_mapping(
                    TOKEN_CORTEX,
                    ConfigMappingRequest::Read {
                        uuid: MAPPING_UUID.to_string(),
                    },
                )
                .await
                .unwrap();
            match response {
                ConfigMappingResponse::Value { message, value } => {
                    assert_eq!(message, "Read flex mapping config successful");
                    assert_eq!(value.name, "Flex Contract");
                }
                other => panic!("unexpected config mapping read response: {other:?}"),
            }
        }
        StepKind::ConfigMappingUpdate => {
            let response = client
                .config_mapping(
                    TOKEN_CORTEX,
                    ConfigMappingRequest::Update {
                        uuid: MAPPING_UUID.to_string(),
                        name: Some("Flex Contract Updated".to_string()),
                        mappings: Some(config_mapping_updated_mappings()),
                    },
                )
                .await
                .unwrap();
            match response {
                ConfigMappingResponse::Value { message, value } => {
                    assert_eq!(message, "Update flex mapping config successful");
                    assert_eq!(value.name, "Flex Contract Updated");
                }
                other => panic!("unexpected config mapping update response: {other:?}"),
            }
        }
        StepKind::ConfigMappingDelete => {
            let response = client
                .config_mapping(
                    TOKEN_CORTEX,
                    ConfigMappingRequest::Delete {
                        uuid: MAPPING_UUID.to_string(),
                    },
                )
                .await
                .unwrap();
            match response {
                ConfigMappingResponse::Deleted { message, uuid } => {
                    assert_eq!(message, "Delete flex mapping config successful");
                    assert_eq!(uuid, MAPPING_UUID);
                }
                other => panic!("unexpected config mapping delete response: {other:?}"),
            }
        }
        StepKind::UpdateHeadset => {
            let result = client
                .update_headset(
                    TOKEN_CORTEX,
                    HEADSET_ID,
                    serde_json::json!({
                        "mode": "EPOCPLUS",
                        "eegRate": 256,
                        "memsRate": 64
                    }),
                )
                .await
                .unwrap();
            assert_eq!(result["headsetId"], HEADSET_ID);
        }
        StepKind::UpdateHeadsetCustomInfo => {
            let result = client
                .update_headset_custom_info(
                    TOKEN_CORTEX,
                    HEADSET_ID,
                    Some(HEADBAND_POSITION),
                    Some(CUSTOM_HEADSET_NAME),
                )
                .await
                .unwrap();
            assert_eq!(result["customName"], CUSTOM_HEADSET_NAME);
        }
        StepKind::SyncWithHeadsetClock => {
            let result = client.sync_with_headset_clock(HEADSET_ID).await.unwrap();
            assert_eq!(result.headset, HEADSET_ID);
            assert!((result.adjustment - 0.0123).abs() < f64::EPSILON);
        }
        StepKind::CreateSession => {
            let session = client
                .create_session(TOKEN_CORTEX, HEADSET_ID)
                .await
                .unwrap();
            assert_eq!(session.id, SESSION_ID);
            assert_eq!(session.status, "activated");
        }
        StepKind::CloseSession => {
            client
                .close_session(TOKEN_CORTEX, SESSION_ID)
                .await
                .unwrap();
        }
        StepKind::QuerySessions => {
            let sessions = client.query_sessions(TOKEN_CORTEX).await.unwrap();
            assert_eq!(sessions.len(), 1);
            assert_eq!(sessions[0].id, SESSION_ID);
            assert_eq!(
                sessions[0]
                    .headset
                    .as_ref()
                    .map(|headset| headset.id.as_str()),
                Some(HEADSET_ID)
            );
        }
        StepKind::SubscribeStreams => {
            let result = client
                .subscribe_streams(TOKEN_CORTEX, SESSION_ID, &CONTRACT_STREAMS)
                .await
                .unwrap();
            assert_eq!(result["success"].as_array().map(Vec::len), Some(3));
            assert_eq!(result["failure"].as_array().map(Vec::len), Some(0));
        }
        StepKind::UnsubscribeStreams => {
            client
                .unsubscribe_streams(TOKEN_CORTEX, SESSION_ID, &CONTRACT_STREAMS)
                .await
                .unwrap();
        }
        StepKind::InjectMarker => {
            let marker = client
                .inject_marker(
                    TOKEN_CORTEX,
                    SESSION_ID,
                    MARKER_LABEL,
                    42,
                    "python-app",
                    Some(12345.0),
                )
                .await
                .unwrap();
            assert_eq!(marker.uuid, MARKER_ID);
            assert_eq!(
                marker.start_datetime.as_deref(),
                Some("2026-02-12T09:01:00Z")
            );
        }
        StepKind::UpdateMarker => {
            client
                .update_marker(TOKEN_CORTEX, SESSION_ID, MARKER_ID, Some(12346.0))
                .await
                .unwrap();
        }
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
        StepKind::HasAccessRight => {
            let granted = client.has_access_right().await.unwrap();
            assert!(granted);
        }
        StepKind::GetUserLogin => {
            let users = client.get_user_login().await.unwrap();
            assert_eq!(users.len(), 1);
            assert_eq!(users[0].username, "contract-user");
            assert_eq!(users[0].logged_in_os_uid.as_deref(), Some("launcher"));
        }
        StepKind::GetUserInfo => {
            let result = client.get_user_info().await.unwrap();
            assert_eq!(result["username"], "contract-user");
            assert_eq!(result["firstName"], "Contract");
        }
        StepKind::GetLicenseInfo => {
            let result = client.get_license_info().await.unwrap();
            assert_eq!(result["isOnline"], true);
            assert_eq!(result["license"]["id"], "license-001");
        }
        StepKind::ConnectHeadset => {
            client.connect_headset(HEADSET_ID).await.unwrap();
        }
        StepKind::DisconnectHeadset => {
            client.disconnect_headset(HEADSET_ID).await.unwrap();
        }
        StepKind::RefreshHeadsets => {
            client.refresh_headsets().await.unwrap();
        }
        StepKind::ConfigMappingCreate => {
            let response = client
                .config_mapping(ConfigMappingRequest::Create {
                    name: "Flex Contract".to_string(),
                    mappings: config_mapping_create_mappings(),
                })
                .await
                .unwrap();
            match response {
                ConfigMappingResponse::Value { message, value } => {
                    assert_eq!(message, "Create flex mapping config successful");
                    assert_eq!(value.uuid, MAPPING_UUID);
                }
                other => panic!("unexpected config mapping create response: {other:?}"),
            }
        }
        StepKind::ConfigMappingGet => {
            let response = client
                .config_mapping(ConfigMappingRequest::Get)
                .await
                .unwrap();
            match response {
                ConfigMappingResponse::List { message, value } => {
                    assert_eq!(message, "Get flex mapping config successful");
                    assert_eq!(value.config.len(), 1);
                    assert_eq!(value.config[0].uuid, MAPPING_UUID);
                }
                other => panic!("unexpected config mapping get response: {other:?}"),
            }
        }
        StepKind::ConfigMappingRead => {
            let response = client
                .config_mapping(ConfigMappingRequest::Read {
                    uuid: MAPPING_UUID.to_string(),
                })
                .await
                .unwrap();
            match response {
                ConfigMappingResponse::Value { message, value } => {
                    assert_eq!(message, "Read flex mapping config successful");
                    assert_eq!(value.name, "Flex Contract");
                }
                other => panic!("unexpected config mapping read response: {other:?}"),
            }
        }
        StepKind::ConfigMappingUpdate => {
            let response = client
                .config_mapping(ConfigMappingRequest::Update {
                    uuid: MAPPING_UUID.to_string(),
                    name: Some("Flex Contract Updated".to_string()),
                    mappings: Some(config_mapping_updated_mappings()),
                })
                .await
                .unwrap();
            match response {
                ConfigMappingResponse::Value { message, value } => {
                    assert_eq!(message, "Update flex mapping config successful");
                    assert_eq!(value.name, "Flex Contract Updated");
                }
                other => panic!("unexpected config mapping update response: {other:?}"),
            }
        }
        StepKind::ConfigMappingDelete => {
            let response = client
                .config_mapping(ConfigMappingRequest::Delete {
                    uuid: MAPPING_UUID.to_string(),
                })
                .await
                .unwrap();
            match response {
                ConfigMappingResponse::Deleted { message, uuid } => {
                    assert_eq!(message, "Delete flex mapping config successful");
                    assert_eq!(uuid, MAPPING_UUID);
                }
                other => panic!("unexpected config mapping delete response: {other:?}"),
            }
        }
        StepKind::UpdateHeadset => {
            let result = client
                .update_headset(
                    HEADSET_ID,
                    serde_json::json!({
                        "mode": "EPOCPLUS",
                        "eegRate": 256,
                        "memsRate": 64
                    }),
                )
                .await
                .unwrap();
            assert_eq!(result["headsetId"], HEADSET_ID);
        }
        StepKind::UpdateHeadsetCustomInfo => {
            let result = client
                .update_headset_custom_info(
                    HEADSET_ID,
                    Some(HEADBAND_POSITION),
                    Some(CUSTOM_HEADSET_NAME),
                )
                .await
                .unwrap();
            assert_eq!(result["customName"], CUSTOM_HEADSET_NAME);
        }
        StepKind::SyncWithHeadsetClock => {
            let result = client.sync_with_headset_clock(HEADSET_ID).await.unwrap();
            assert_eq!(result.headset, HEADSET_ID);
            assert!((result.adjustment - 0.0123).abs() < f64::EPSILON);
        }
        StepKind::CreateSession => {
            let session = client.create_session(HEADSET_ID).await.unwrap();
            assert_eq!(session.id, SESSION_ID);
            assert_eq!(session.status, "activated");
        }
        StepKind::CloseSession => {
            client.close_session(SESSION_ID).await.unwrap();
        }
        StepKind::QuerySessions => {
            let sessions = client.query_sessions().await.unwrap();
            assert_eq!(sessions.len(), 1);
            assert_eq!(sessions[0].id, SESSION_ID);
            assert_eq!(
                sessions[0]
                    .headset
                    .as_ref()
                    .map(|headset| headset.id.as_str()),
                Some(HEADSET_ID)
            );
        }
        StepKind::SubscribeStreams => {
            client
                .subscribe_streams(SESSION_ID, &CONTRACT_STREAMS)
                .await
                .unwrap();
        }
        StepKind::UnsubscribeStreams => {
            client
                .unsubscribe_streams(SESSION_ID, &CONTRACT_STREAMS)
                .await
                .unwrap();
        }
        StepKind::InjectMarker => {
            let marker = client
                .inject_marker(SESSION_ID, MARKER_LABEL, 42, "python-app", Some(12345.0))
                .await
                .unwrap();
            assert_eq!(marker.uuid, MARKER_ID);
            assert_eq!(
                marker.start_datetime.as_deref(),
                Some("2026-02-12T09:01:00Z")
            );
        }
        StepKind::UpdateMarker => {
            client
                .update_marker(SESSION_ID, MARKER_ID, Some(12346.0))
                .await
                .unwrap();
        }
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
