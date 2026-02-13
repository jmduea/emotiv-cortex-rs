//! Protocol constants for method names, error codes, and stream names.

/// Known Cortex API method names.
pub struct Methods;

impl Methods {
    // --- Cortex info ------------------------------------------------

    /// Get Cortex version and build info.
    pub const GET_CORTEX_INFO: &'static str = "getCortexInfo";

    // ─── Authentication ─────────────────────────────────────────────

    /// Get the currently logged-in user.
    pub const GET_USER_LOGIN: &'static str = "getUserLogin";

    /// Request application access from the user.
    pub const REQUEST_ACCESS: &'static str = "requestAccess";

    /// Check if the app has been granted access.
    pub const HAS_ACCESS_RIGHT: &'static str = "hasAccessRight";

    /// Authorize and obtain a cortex token.
    pub const AUTHORIZE: &'static str = "authorize";

    /// Generate a new cortex token.
    /// Can also be used to refresh an existing token by providing the current token in the params.
    pub const GENERATE_NEW_TOKEN: &'static str = "generateNewToken";

    /// Get basic information about the current user.
    pub const GET_USER_INFO: &'static str = "getUserInformation";

    /// Get information about the license currently used by your app
    pub const GET_LICENSE_INFO: &'static str = "getLicenseInfo";

    // ─── Headset Management ─────────────────────────────────────────

    /// Control (connect/disconnect/refresh) a specific headset.
    pub const CONTROL_DEVICE: &'static str = "controlDevice";

    /// Manage EEG channel mapping configs for an EPOC Flex headset
    pub const CONFIG_MAPPING: &'static str = "configMapping";

    /// Query available headsets.
    pub const QUERY_HEADSETS: &'static str = "queryHeadsets";

    /// Update settings of an EPOC+ or EPOC X headset.
    pub const UPDATE_HEADSET: &'static str = "updateHeadset";

    /// Update headband position or custom info of an EPOC X headset.
    pub const UPDATE_HEADSET_CUSTOM_INFO: &'static str = "updateHeadsetCustomInfo";

    /// Synchronize system time with headset clock.
    pub const SYNC_WITH_HEADSET_CLOCK: &'static str = "syncWithHeadsetClock";

    // ─── Session Management ─────────────────────────────────────────
    /// Create a session (associates a headset with a cortex token).
    pub const CREATE_SESSION: &'static str = "createSession";

    /// Update a session (activate, close, etc.).
    pub const UPDATE_SESSION: &'static str = "updateSession";

    /// Query existing sessions.
    pub const QUERY_SESSIONS: &'static str = "querySessions";

    // ─── Data Streams ───────────────────────────────────────────────
    /// Subscribe to data streams (eeg, dev, mot, fac, etc.).
    pub const SUBSCRIBE: &'static str = "subscribe";

    /// Unsubscribe from data streams.
    pub const UNSUBSCRIBE: &'static str = "unsubscribe";

    // ─── Records ────────────────────────────────────────────────────
    /// Create a new data recording.
    pub const CREATE_RECORD: &'static str = "createRecord";

    /// Stop an active recording.
    pub const STOP_RECORD: &'static str = "stopRecord";

    /// Update a recording's metadata.
    pub const UPDATE_RECORD: &'static str = "updateRecord";

    /// Delete one or more recordings.
    pub const DELETE_RECORD: &'static str = "deleteRecord";

    /// Export a recording to CSV/EDF.
    pub const EXPORT_RECORD: &'static str = "exportRecord";

    /// Query recorded sessions.
    pub const QUERY_RECORDS: &'static str = "queryRecords";

    /// Get a list of records, selected by their id
    pub const GET_RECORD_INFOS: &'static str = "getRecordInfos";

    /// Configure the opt-out feature for records.
    /// This handles whether or not records created are automatically shared with Emotiv for research purposes.
    /// By default, records are NOT shared with Emotiv.
    pub const CONFIG_OPT_OUT: &'static str = "configOptOut";

    /// Download a record from Emotiv cloud
    pub const DOWNLOAD_RECORD: &'static str = "requestToDownloadRecordData";

    // ─── Markers ────────────────────────────────────────────────────
    /// Inject a time-stamped marker during recording.
    pub const INJECT_MARKER: &'static str = "injectMarker";

    /// Update a marker (convert instance to interval marker).
    pub const UPDATE_MARKER: &'static str = "updateMarker";

    // --- Subjects ---------------------------------------------------

    /// Create a new subject (user) in the EMOTIV system.
    pub const CREATE_SUBJECT: &'static str = "createSubject";

    /// Update an existing subject's information.
    pub const UPDATE_SUBJECT: &'static str = "updateSubject";

    /// Delete one or more subjects.
    pub const DELETE_SUBJECTS: &'static str = "deleteSubjects";

    /// Query existing subjects.
    pub const QUERY_SUBJECTS: &'static str = "querySubjects";

    /// Get demographic attributes
    pub const GET_DEMOGRAPHIC_ATTRIBUTES: &'static str = "getDemographicAttributes";

    // ─── BCI/Training/Profiles ───────────────────────────────────────────────────
    /// List user profiles.
    pub const QUERY_PROFILE: &'static str = "queryProfile";

    /// Get the profile loaded for a headset.
    pub const GET_CURRENT_PROFILE: &'static str = "getCurrentProfile";

    /// Manage profiles (create, load, unload, save, rename, delete).
    pub const SETUP_PROFILE: &'static str = "setupProfile";

    /// Load an empty profile for a headset
    pub const LOAD_GUEST_PROFILE: &'static str = "loadGuestProfile";

    /// Control training lifecycle (start, accept, reject, reset, erase).
    pub const TRAINING: &'static str = "training";

    /// Get available actions/controls/events for a detection type.
    pub const GET_DETECTION_INFO: &'static str = "getDetectionInfo";

    // --- Advanced BCI -------------------------------------------------------------

    /// Get a list of trained actions of a profile.
    pub const GET_TRAINED_SIGNATURE_ACTIONS: &'static str = "getTrainedSignatureActions";

    /// Get the duration of a training
    pub const GET_TRAINING_TIME: &'static str = "getTrainingTime";

    /// Get or set the signature used by the facial expression detection.
    pub const FACIAL_EXPRESSION_SIGNATURE_TYPE: &'static str = "facialExpressionSignatureType";

    /// Get or set the threshold of a facial expression action for a profile.
    pub const FACIAL_EXPRESSION_THRESHOLD: &'static str = "facialExpressionThreshold";

    /// Get or set active mental command actions.
    pub const MENTAL_COMMAND_ACTIVE_ACTION: &'static str = "mentalCommandActiveAction";

    /// Get mental command brain mapping data.
    pub const MENTAL_COMMAND_BRAIN_MAP: &'static str = "mentalCommandBrainMap";

    /// Get or set mental command training threshold.
    pub const MENTAL_COMMAND_TRAINING_THRESHOLD: &'static str = "mentalCommandTrainingThreshold";

    /// Get or set mental command action sensitivity.
    pub const MENTAL_COMMAND_ACTION_SENSITIVITY: &'static str = "mentalCommandActionSensitivity";
}

// ─── Error Codes ────────────────────────────────────────────────────────

/// Cortex API error codes.
pub struct ErrorCodes;

impl ErrorCodes {
    // ─── JSON-RPC standard errors ────────────────────────────────────
    /// Method not found (unknown or deprecated method name).
    pub const METHOD_NOT_FOUND: i32 = -32601;

    // ─── Cortex-specific errors ──────────────────────────────────────
    /// No headset connected.
    pub const NO_HEADSET_CONNECTED: i32 = -32001;

    /// Invalid license ID.
    pub const INVALID_LICENSE_ID: i32 = -32002;

    /// Headset unavailable.
    pub const HEADSET_UNAVAILABLE: i32 = -32004;

    /// Session already exists.
    pub const SESSION_ALREADY_EXISTS: i32 = -32005;

    /// Session must be activated before this operation.
    pub const SESSION_MUST_BE_ACTIVATED: i32 = -32012;

    /// Invalid cortex token.
    pub const INVALID_CORTEX_TOKEN: i32 = -32014;

    /// Cortex token expired.
    pub const TOKEN_EXPIRED: i32 = -32015;

    /// Invalid stream for subscribe/unsubscribe.
    pub const INVALID_STREAM: i32 = -32016;

    /// Invalid client credentials.
    pub const INVALID_CLIENT_CREDENTIALS: i32 = -32021;

    /// License expired or unavailable.
    pub const LICENSE_EXPIRED: i32 = -32024;

    /// User not logged in to EmotivID in the Launcher.
    pub const USER_NOT_LOGGED_IN: i32 = -32033;

    /// Application is unpublished/unapproved for this account.
    pub const UNPUBLISHED_APPLICATION: i32 = -32142;

    /// Headset not ready yet.
    pub const HEADSET_NOT_READY: i32 = -32152;

    // Backward-compatible aliases for older naming.
    pub const ACCESS_DENIED: i32 = Self::INVALID_LICENSE_ID;
    pub const HEADSET_IN_USE: i32 = Self::SESSION_MUST_BE_ACTIVATED;
    pub const NOT_APPROVED: i32 = Self::UNPUBLISHED_APPLICATION;
    pub const CORTEX_STARTING: i32 = Self::HEADSET_NOT_READY;
}

// ─── Stream Names ───────────────────────────────────────────────────────

/// Known Cortex data stream names for subscribe/unsubscribe.
pub struct Streams;

impl Streams {
    /// Raw EEG channel data (Premium API).
    pub const EEG: &'static str = "eeg";
    /// Device status: battery, signal, contact quality.
    pub const DEV: &'static str = "dev";
    /// Motion/IMU: accelerometer, magnetometer, gyroscope/quaternion.
    pub const MOT: &'static str = "mot";
    /// EEG quality per sensor.
    pub const EQ: &'static str = "eq";
    /// Band power: theta/alpha/betaL/betaH/gamma per channel.
    pub const POW: &'static str = "pow";
    /// Performance metrics: attention, stress, engagement, etc.
    pub const MET: &'static str = "met";
    /// Mental commands: action + power (requires profile).
    pub const COM: &'static str = "com";
    /// Facial expressions: eye/face actions + power.
    pub const FAC: &'static str = "fac";
    /// System/training events.
    pub const SYS: &'static str = "sys";

    /// All available stream names.
    pub const ALL: &'static [&'static str] = &[
        Self::EEG,
        Self::DEV,
        Self::MOT,
        Self::EQ,
        Self::POW,
        Self::MET,
        Self::COM,
        Self::FAC,
        Self::SYS,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_user_info_method_name() {
        assert_eq!(Methods::GET_USER_INFO, "getUserInformation");
    }

    #[test]
    fn test_streams_all_invariants() {
        use std::collections::HashSet;

        let all = Streams::ALL;
        assert_eq!(all.len(), 9);

        let unique: HashSet<&str> = all.iter().copied().collect();
        assert_eq!(unique.len(), all.len(), "Streams::ALL contains duplicates");

        assert!(unique.contains(Streams::EEG));
        assert!(unique.contains(Streams::DEV));
        assert!(unique.contains(Streams::MOT));
        assert!(unique.contains(Streams::EQ));
        assert!(unique.contains(Streams::POW));
        assert!(unique.contains(Streams::MET));
        assert!(unique.contains(Streams::COM));
        assert!(unique.contains(Streams::FAC));
        assert!(unique.contains(Streams::SYS));
    }
}
