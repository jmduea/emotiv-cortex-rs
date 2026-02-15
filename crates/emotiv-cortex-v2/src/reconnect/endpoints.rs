use crate::error::CortexResult;
use crate::protocol::auth::UserLoginInfo;
use crate::protocol::headset::{
    ConfigMappingRequest, ConfigMappingResponse, HeadsetClockSyncResult, HeadsetInfo,
    QueryHeadsetsOptions,
};
use crate::protocol::profiles::{CurrentProfileInfo, ProfileAction, ProfileInfo};
use crate::protocol::records::{ExportFormat, MarkerInfo, RecordInfo, UpdateRecordRequest};
use crate::protocol::session::SessionInfo;
use crate::protocol::subjects::{
    DemographicAttribute, QuerySubjectsRequest, SubjectInfo, SubjectRequest,
};
use crate::protocol::training::{
    DetectionInfo, DetectionType, FacialExpressionSignatureTypeRequest,
    FacialExpressionThresholdRequest, MentalCommandTrainingThresholdRequest,
    TrainedSignatureActions, TrainingStatus, TrainingTime,
};

use super::ResilientClient;

impl ResilientClient {
    // ─── Authentication ─────────────────────────────────────────────────

    /// Query Cortex service info. No authentication required.
    pub async fn get_cortex_info(&self) -> CortexResult<serde_json::Value> {
        self.exec(|c| async move { c.get_cortex_info().await })
            .await
    }

    /// Check if the application has access rights.
    pub async fn has_access_right(&self) -> CortexResult<bool> {
        let client_id = self.config.client_id.clone();
        let client_secret = self.config.client_secret.clone();
        self.exec(move |c| {
            let id = client_id.clone();
            let secret = client_secret.clone();
            async move { c.has_access_right(&id, &secret).await }
        })
        .await
    }

    /// Get the currently logged-in Emotiv user.
    pub async fn get_user_login(&self) -> CortexResult<Vec<UserLoginInfo>> {
        self.exec(|c| async move { c.get_user_login().await }).await
    }

    /// Get information about the current user.
    pub async fn get_user_info(&self) -> CortexResult<serde_json::Value> {
        self.exec_with_token(|c, token| async move { c.get_user_info(&token).await })
            .await
    }

    /// Get information about the license used by the application.
    pub async fn get_license_info(&self) -> CortexResult<serde_json::Value> {
        self.exec_with_token(|c, token| async move { c.get_license_info(&token).await })
            .await
    }

    // ─── Headset Management ─────────────────────────────────────────────

    /// Query available headsets.
    pub async fn query_headsets(
        &self,
        options: QueryHeadsetsOptions,
    ) -> CortexResult<Vec<HeadsetInfo>> {
        self.exec(move |c| {
            let options = options.clone();
            async move { c.query_headsets(options).await }
        })
        .await
    }

    /// Connect to a headset.
    pub async fn connect_headset(&self, headset_id: &str) -> CortexResult<()> {
        let id = headset_id.to_string();
        self.exec(move |c| {
            let id = id.clone();
            async move { c.connect_headset(&id).await }
        })
        .await
    }

    /// Disconnect a headset.
    pub async fn disconnect_headset(&self, headset_id: &str) -> CortexResult<()> {
        let id = headset_id.to_string();
        self.exec(move |c| {
            let id = id.clone();
            async move { c.disconnect_headset(&id).await }
        })
        .await
    }

    /// Trigger headset scanning / refresh.
    pub async fn refresh_headsets(&self) -> CortexResult<()> {
        self.exec(|c| async move { c.refresh_headsets().await })
            .await
    }

    /// Synchronize system clock with headset clock.
    pub async fn sync_with_headset_clock(
        &self,
        headset_id: &str,
    ) -> CortexResult<HeadsetClockSyncResult> {
        let id = headset_id.to_string();
        self.exec(move |c| {
            let id = id.clone();
            async move { c.sync_with_headset_clock(&id).await }
        })
        .await
    }

    /// Manage EEG channel mapping configurations for an EPOC Flex headset.
    pub async fn config_mapping(
        &self,
        request: ConfigMappingRequest,
    ) -> CortexResult<ConfigMappingResponse> {
        self.exec_with_token(move |c, token| {
            let request = request.clone();
            async move { c.config_mapping(&token, request).await }
        })
        .await
    }

    /// Update settings of an EPOC+ or EPOC X headset.
    pub async fn update_headset(
        &self,
        headset_id: &str,
        setting: serde_json::Value,
    ) -> CortexResult<serde_json::Value> {
        let id = headset_id.to_string();
        self.exec_with_token(move |c, token| {
            let id = id.clone();
            let setting = setting.clone();
            async move { c.update_headset(&token, &id, setting).await }
        })
        .await
    }

    /// Update the headband position or custom name of an EPOC X headset.
    pub async fn update_headset_custom_info(
        &self,
        headset_id: &str,
        headband_position: Option<&str>,
        custom_name: Option<&str>,
    ) -> CortexResult<serde_json::Value> {
        let id = headset_id.to_string();
        let pos = headband_position.map(|s| s.to_string());
        let name = custom_name.map(|s| s.to_string());
        self.exec_with_token(move |c, token| {
            let id = id.clone();
            let pos = pos.clone();
            let name = name.clone();
            async move {
                c.update_headset_custom_info(&token, &id, pos.as_deref(), name.as_deref())
                    .await
            }
        })
        .await
    }

    // ─── Session Management ─────────────────────────────────────────────

    /// Create a session for a headset.
    pub async fn create_session(&self, headset_id: &str) -> CortexResult<SessionInfo> {
        let id = headset_id.to_string();
        self.exec_with_token(move |c, token| {
            let id = id.clone();
            async move { c.create_session(&token, &id).await }
        })
        .await
    }

    /// Query existing sessions.
    pub async fn query_sessions(&self) -> CortexResult<Vec<SessionInfo>> {
        self.exec_with_token(|c, token| async move { c.query_sessions(&token).await })
            .await
    }

    /// Close a session.
    pub async fn close_session(&self, session_id: &str) -> CortexResult<()> {
        let id = session_id.to_string();
        self.exec_with_token(move |c, token| {
            let id = id.clone();
            async move { c.close_session(&token, &id).await }
        })
        .await
    }

    // ─── Data Streams ───────────────────────────────────────────────────

    /// Create data stream channels for the specified streams.
    ///
    /// This delegates to the underlying
    /// [`crate::client::CortexClient::create_stream_channels`].
    pub async fn create_stream_channels(&self, streams: &[&str]) -> crate::client::StreamReceivers {
        self.client().await.create_stream_channels(streams)
    }

    /// Subscribe to data streams.
    pub async fn subscribe_streams(&self, session_id: &str, streams: &[&str]) -> CortexResult<()> {
        let sid = session_id.to_string();
        let stream_names: Vec<String> = streams.iter().map(|s| s.to_string()).collect();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            let names = stream_names.clone();
            async move {
                let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
                c.subscribe_streams(&token, &sid, &refs).await
            }
        })
        .await
    }

    /// Unsubscribe from data streams.
    pub async fn unsubscribe_streams(
        &self,
        session_id: &str,
        streams: &[&str],
    ) -> CortexResult<()> {
        let sid = session_id.to_string();
        let stream_names: Vec<String> = streams.iter().map(|s| s.to_string()).collect();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            let names = stream_names.clone();
            async move {
                let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
                c.unsubscribe_streams(&token, &sid, &refs).await
            }
        })
        .await
    }

    // ─── Records ────────────────────────────────────────────────────────

    /// Start a new recording.
    pub async fn create_record(&self, session_id: &str, title: &str) -> CortexResult<RecordInfo> {
        let sid = session_id.to_string();
        let t = title.to_string();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            let t = t.clone();
            async move { c.create_record(&token, &sid, &t).await }
        })
        .await
    }

    /// Stop an active recording.
    pub async fn stop_record(&self, session_id: &str) -> CortexResult<RecordInfo> {
        let sid = session_id.to_string();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            async move { c.stop_record(&token, &sid).await }
        })
        .await
    }

    /// Query recorded sessions.
    pub async fn query_records(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> CortexResult<Vec<RecordInfo>> {
        self.exec_with_token(
            move |c, token| async move { c.query_records(&token, limit, offset).await },
        )
        .await
    }

    /// Export a recording to CSV or EDF format.
    pub async fn export_record(
        &self,
        record_ids: &[String],
        folder: &str,
        format: ExportFormat,
    ) -> CortexResult<()> {
        let ids = record_ids.to_vec();
        let f = folder.to_string();
        self.exec_with_token(move |c, token| {
            let ids = ids.clone();
            let f = f.clone();
            async move { c.export_record(&token, &ids, &f, format).await }
        })
        .await
    }

    /// Update a recording's metadata (title, description, tags).
    pub async fn update_record_with(
        &self,
        request: &UpdateRecordRequest,
    ) -> CortexResult<RecordInfo> {
        let request = request.clone();
        self.exec_with_token(move |c, token| {
            let request = request.clone();
            async move { c.update_record_with(&token, &request).await }
        })
        .await
    }

    /// Update a recording's metadata (title, description, tags).
    #[deprecated(note = "Use `update_record_with` and `UpdateRecordRequest` instead.")]
    pub async fn update_record(
        &self,
        record_id: &str,
        title: Option<&str>,
        description: Option<&str>,
        tags: Option<&[String]>,
    ) -> CortexResult<RecordInfo> {
        let request = UpdateRecordRequest {
            record_id: record_id.to_string(),
            title: title.map(ToString::to_string),
            description: description.map(ToString::to_string),
            tags: tags.map(|values| values.to_vec()),
        };
        self.update_record_with(&request).await
    }

    /// Delete one or more recordings.
    pub async fn delete_record(&self, record_ids: &[String]) -> CortexResult<serde_json::Value> {
        let ids = record_ids.to_vec();
        self.exec_with_token(move |c, token| {
            let ids = ids.clone();
            async move { c.delete_record(&token, &ids).await }
        })
        .await
    }

    /// Get detailed information for specific records by their IDs.
    pub async fn get_record_infos(&self, record_ids: &[String]) -> CortexResult<serde_json::Value> {
        let ids = record_ids.to_vec();
        self.exec_with_token(move |c, token| {
            let ids = ids.clone();
            async move { c.get_record_infos(&token, &ids).await }
        })
        .await
    }

    /// Configure the opt-out setting for data sharing.
    pub async fn config_opt_out(
        &self,
        status: &str,
        new_opt_out: Option<bool>,
    ) -> CortexResult<serde_json::Value> {
        let st = status.to_string();
        self.exec_with_token(move |c, token| {
            let st = st.clone();
            async move { c.config_opt_out(&token, &st, new_opt_out).await }
        })
        .await
    }

    /// Request to download recorded data from the Emotiv cloud.
    pub async fn download_record(&self, record_ids: &[String]) -> CortexResult<serde_json::Value> {
        let ids = record_ids.to_vec();
        self.exec_with_token(move |c, token| {
            let ids = ids.clone();
            async move { c.download_record(&token, &ids).await }
        })
        .await
    }

    // ─── Markers ────────────────────────────────────────────────────────

    /// Inject a time-stamped marker.
    pub async fn inject_marker(
        &self,
        session_id: &str,
        label: &str,
        value: i32,
        port: &str,
        time: Option<f64>,
    ) -> CortexResult<MarkerInfo> {
        let sid = session_id.to_string();
        let l = label.to_string();
        let p = port.to_string();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            let l = l.clone();
            let p = p.clone();
            async move { c.inject_marker(&token, &sid, &l, value, &p, time).await }
        })
        .await
    }

    /// Update a marker (convert instance to interval marker).
    pub async fn update_marker(
        &self,
        session_id: &str,
        marker_id: &str,
        time: Option<f64>,
    ) -> CortexResult<()> {
        let sid = session_id.to_string();
        let mid = marker_id.to_string();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            let mid = mid.clone();
            async move { c.update_marker(&token, &sid, &mid, time).await }
        })
        .await
    }

    // ─── Subjects ────────────────────────────────────────────────────────

    /// Create a new subject.
    pub async fn create_subject_with(&self, request: &SubjectRequest) -> CortexResult<SubjectInfo> {
        let request = request.clone();
        self.exec_with_token(move |c, token| {
            let request = request.clone();
            async move { c.create_subject_with(&token, &request).await }
        })
        .await
    }

    /// Create a new subject.
    #[deprecated(note = "Use `create_subject_with` and `SubjectRequest` instead.")]
    #[allow(clippy::too_many_arguments)]
    pub async fn create_subject(
        &self,
        subject_name: &str,
        date_of_birth: Option<&str>,
        sex: Option<&str>,
        country_code: Option<&str>,
        state: Option<&str>,
        city: Option<&str>,
        attributes: Option<&[serde_json::Value]>,
    ) -> CortexResult<SubjectInfo> {
        let request = SubjectRequest {
            subject_name: subject_name.to_string(),
            date_of_birth: date_of_birth.map(ToString::to_string),
            sex: sex.map(ToString::to_string),
            country_code: country_code.map(ToString::to_string),
            state: state.map(ToString::to_string),
            city: city.map(ToString::to_string),
            attributes: attributes.map(|values| values.to_vec()),
        };
        self.create_subject_with(&request).await
    }

    /// Update an existing subject's information.
    pub async fn update_subject_with(&self, request: &SubjectRequest) -> CortexResult<SubjectInfo> {
        let request = request.clone();
        self.exec_with_token(move |c, token| {
            let request = request.clone();
            async move { c.update_subject_with(&token, &request).await }
        })
        .await
    }

    /// Update an existing subject's information.
    #[deprecated(note = "Use `update_subject_with` and `SubjectRequest` instead.")]
    #[allow(clippy::too_many_arguments)]
    pub async fn update_subject(
        &self,
        subject_name: &str,
        date_of_birth: Option<&str>,
        sex: Option<&str>,
        country_code: Option<&str>,
        state: Option<&str>,
        city: Option<&str>,
        attributes: Option<&[serde_json::Value]>,
    ) -> CortexResult<SubjectInfo> {
        let request = SubjectRequest {
            subject_name: subject_name.to_string(),
            date_of_birth: date_of_birth.map(ToString::to_string),
            sex: sex.map(ToString::to_string),
            country_code: country_code.map(ToString::to_string),
            state: state.map(ToString::to_string),
            city: city.map(ToString::to_string),
            attributes: attributes.map(|values| values.to_vec()),
        };
        self.update_subject_with(&request).await
    }

    /// Delete one or more subjects.
    pub async fn delete_subjects(
        &self,
        subject_names: &[String],
    ) -> CortexResult<serde_json::Value> {
        let names = subject_names.to_vec();
        self.exec_with_token(move |c, token| {
            let names = names.clone();
            async move { c.delete_subjects(&token, &names).await }
        })
        .await
    }

    /// Query subjects with filtering, sorting, and pagination.
    pub async fn query_subjects_with(
        &self,
        request: &QuerySubjectsRequest,
    ) -> CortexResult<(Vec<SubjectInfo>, u32)> {
        let request = request.clone();
        self.exec_with_token(move |c, token| {
            let request = request.clone();
            async move { c.query_subjects_with(&token, &request).await }
        })
        .await
    }

    /// Query subjects with filtering, sorting, and pagination.
    #[deprecated(note = "Use `query_subjects_with` and `QuerySubjectsRequest` instead.")]
    pub async fn query_subjects(
        &self,
        query: serde_json::Value,
        order_by: serde_json::Value,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> CortexResult<(Vec<SubjectInfo>, u32)> {
        let request = QuerySubjectsRequest {
            query,
            order_by,
            limit,
            offset,
        };
        self.query_subjects_with(&request).await
    }

    /// Get the list of valid demographic attributes.
    pub async fn get_demographic_attributes(&self) -> CortexResult<Vec<DemographicAttribute>> {
        self.exec_with_token(|c, token| async move { c.get_demographic_attributes(&token).await })
            .await
    }

    // ─── Profiles ───────────────────────────────────────────────────────

    /// List all profiles for the current user.
    pub async fn query_profiles(&self) -> CortexResult<Vec<ProfileInfo>> {
        self.exec_with_token(|c, token| async move { c.query_profiles(&token).await })
            .await
    }

    /// Get the profile currently loaded for a headset.
    pub async fn get_current_profile(&self, headset_id: &str) -> CortexResult<CurrentProfileInfo> {
        let id = headset_id.to_string();
        self.exec_with_token(move |c, token| {
            let id = id.clone();
            async move { c.get_current_profile(&token, &id).await }
        })
        .await
    }

    /// Manage a profile (create, load, unload, save, rename, delete).
    pub async fn setup_profile(
        &self,
        headset_id: &str,
        profile_name: &str,
        action: ProfileAction,
    ) -> CortexResult<()> {
        let hid = headset_id.to_string();
        let pname = profile_name.to_string();
        self.exec_with_token(move |c, token| {
            let hid = hid.clone();
            let pname = pname.clone();
            async move { c.setup_profile(&token, &hid, &pname, action).await }
        })
        .await
    }

    /// Load an empty guest profile for a headset.
    pub async fn load_guest_profile(&self, headset_id: &str) -> CortexResult<()> {
        let id = headset_id.to_string();
        self.exec_with_token(move |c, token| {
            let id = id.clone();
            async move { c.load_guest_profile(&token, &id).await }
        })
        .await
    }

    // ─── BCI / Training ─────────────────────────────────────────────────

    /// Get detection info for a detection type.
    pub async fn get_detection_info(
        &self,
        detection: DetectionType,
    ) -> CortexResult<DetectionInfo> {
        self.exec(move |c| async move { c.get_detection_info(detection).await })
            .await
    }

    /// Control the training lifecycle.
    pub async fn training(
        &self,
        session_id: &str,
        detection: DetectionType,
        status: TrainingStatus,
        action: &str,
    ) -> CortexResult<serde_json::Value> {
        let sid = session_id.to_string();
        let act = action.to_string();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            let act = act.clone();
            async move { c.training(&token, &sid, detection, status, &act).await }
        })
        .await
    }

    /// Get or set active mental command actions.
    pub async fn mental_command_active_action(
        &self,
        session_id: &str,
        actions: Option<&[&str]>,
    ) -> CortexResult<serde_json::Value> {
        let sid = session_id.to_string();
        let owned_actions: Option<Vec<String>> =
            actions.map(|a| a.iter().map(|s| s.to_string()).collect());
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            let owned_actions = owned_actions.clone();
            async move {
                let refs: Option<Vec<&str>> = owned_actions
                    .as_ref()
                    .map(|v| v.iter().map(|s| s.as_str()).collect());
                c.mental_command_active_action(&token, &sid, refs.as_deref())
                    .await
            }
        })
        .await
    }

    /// Get or set the mental command action sensitivity.
    pub async fn mental_command_action_sensitivity(
        &self,
        session_id: &str,
        values: Option<&[i32]>,
    ) -> CortexResult<serde_json::Value> {
        let sid = session_id.to_string();
        let owned_values: Option<Vec<i32>> = values.map(|v| v.to_vec());
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            let owned_values = owned_values.clone();
            async move {
                c.mental_command_action_sensitivity(&token, &sid, owned_values.as_deref())
                    .await
            }
        })
        .await
    }

    /// Get the mental command brain map.
    pub async fn mental_command_brain_map(
        &self,
        session_id: &str,
    ) -> CortexResult<serde_json::Value> {
        let sid = session_id.to_string();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            async move { c.mental_command_brain_map(&token, &sid).await }
        })
        .await
    }

    /// Get or set the mental command training threshold.
    pub async fn mental_command_training_threshold(
        &self,
        session_id: &str,
    ) -> CortexResult<serde_json::Value> {
        let sid = session_id.to_string();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            async move { c.mental_command_training_threshold(&token, &sid).await }
        })
        .await
    }

    /// Get or set the mental command training threshold for a profile.
    pub async fn mental_command_training_threshold_for_profile(
        &self,
        profile: &str,
        status: Option<&str>,
        value: Option<f64>,
    ) -> CortexResult<serde_json::Value> {
        let profile_name = profile.to_string();
        let st = status.map(|s| s.to_string());
        self.exec_with_token(move |c, token| {
            let profile_name = profile_name.clone();
            let st = st.clone();
            async move {
                c.mental_command_training_threshold_for_profile(
                    &token,
                    &profile_name,
                    st.as_deref(),
                    value,
                )
                .await
            }
        })
        .await
    }

    /// Get or set the mental command training threshold with a typed request.
    pub async fn mental_command_training_threshold_with_request(
        &self,
        request: &MentalCommandTrainingThresholdRequest,
    ) -> CortexResult<serde_json::Value> {
        let request = request.clone();
        self.exec_with_token(move |c, token| {
            let request = request.clone();
            async move {
                c.mental_command_training_threshold_with_request(&token, &request)
                    .await
            }
        })
        .await
    }

    /// Get or set the mental command training threshold with explicit
    /// session/profile targeting.
    #[deprecated(
        note = "Use `mental_command_training_threshold_with_request` and `MentalCommandTrainingThresholdRequest` instead."
    )]
    pub async fn mental_command_training_threshold_with_params(
        &self,
        session_id: Option<&str>,
        profile: Option<&str>,
        status: Option<&str>,
        value: Option<f64>,
    ) -> CortexResult<serde_json::Value> {
        let request = MentalCommandTrainingThresholdRequest {
            session_id: session_id.map(ToString::to_string),
            profile: profile.map(ToString::to_string),
            status: status.map(ToString::to_string),
            value,
        };
        self.mental_command_training_threshold_with_request(&request)
            .await
    }

    /// Get a list of trained actions for a profile's detection type.
    pub async fn get_trained_signature_actions(
        &self,
        detection: DetectionType,
        profile: Option<&str>,
        session: Option<&str>,
    ) -> CortexResult<TrainedSignatureActions> {
        let p = profile.map(|s| s.to_string());
        let s = session.map(|s| s.to_string());
        self.exec_with_token(move |c, token| {
            let p = p.clone();
            let s = s.clone();
            async move {
                c.get_trained_signature_actions(&token, detection, p.as_deref(), s.as_deref())
                    .await
            }
        })
        .await
    }

    /// Get the duration of a training session.
    pub async fn get_training_time(
        &self,
        detection: DetectionType,
        session_id: &str,
    ) -> CortexResult<TrainingTime> {
        let sid = session_id.to_string();
        self.exec_with_token(move |c, token| {
            let sid = sid.clone();
            async move { c.get_training_time(&token, detection, &sid).await }
        })
        .await
    }

    /// Get or set the facial expression signature type.
    pub async fn facial_expression_signature_type_with(
        &self,
        request: &FacialExpressionSignatureTypeRequest,
    ) -> CortexResult<serde_json::Value> {
        let request = request.clone();
        self.exec_with_token(move |c, token| {
            let request = request.clone();
            async move {
                c.facial_expression_signature_type_with(&token, &request)
                    .await
            }
        })
        .await
    }

    /// Get or set the facial expression signature type.
    #[deprecated(
        note = "Use `facial_expression_signature_type_with` and `FacialExpressionSignatureTypeRequest` instead."
    )]
    pub async fn facial_expression_signature_type(
        &self,
        status: &str,
        profile: Option<&str>,
        session: Option<&str>,
        signature: Option<&str>,
    ) -> CortexResult<serde_json::Value> {
        let request = FacialExpressionSignatureTypeRequest {
            status: status.to_string(),
            profile: profile.map(ToString::to_string),
            session: session.map(ToString::to_string),
            signature: signature.map(ToString::to_string),
        };
        self.facial_expression_signature_type_with(&request).await
    }

    /// Get or set the threshold of a facial expression action.
    pub async fn facial_expression_threshold_with(
        &self,
        request: &FacialExpressionThresholdRequest,
    ) -> CortexResult<serde_json::Value> {
        let request = request.clone();
        self.exec_with_token(move |c, token| {
            let request = request.clone();
            async move { c.facial_expression_threshold_with(&token, &request).await }
        })
        .await
    }

    /// Get or set the threshold of a facial expression action.
    #[deprecated(
        note = "Use `facial_expression_threshold_with` and `FacialExpressionThresholdRequest` instead."
    )]
    pub async fn facial_expression_threshold(
        &self,
        status: &str,
        action: &str,
        profile: Option<&str>,
        session: Option<&str>,
        value: Option<u32>,
    ) -> CortexResult<serde_json::Value> {
        let request = FacialExpressionThresholdRequest {
            status: status.to_string(),
            action: action.to_string(),
            profile: profile.map(ToString::to_string),
            session: session.map(ToString::to_string),
            value,
        };
        self.facial_expression_threshold_with(&request).await
    }
}
