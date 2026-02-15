//! Session management protocol types.

use serde::Deserialize;

use crate::protocol::headset::HeadsetInfo;

/// Session information from `createSession` / `querySessions`.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionInfo {
    /// Session ID (UUID).
    pub id: String,

    /// Session status: "opened", "activated".
    pub status: String,

    /// `EmotivID` of the user
    pub owner: String,

    /// Id of license used by the session
    pub license: String,

    /// Application ID.
    #[serde(rename = "appId")]
    pub app_id: String,

    /// ISO datetime when the session was created.
    pub started: String,

    /// ISO datetime when the session was closed.
    pub stopped: Option<String>,

    /// Data streams subscribed to in this session.
    pub streams: Vec<String>,

    /// Ids of all records created during this session.
    #[serde(rename = "recordIds")]
    pub record_ids: Vec<String>,

    /// Whether this session is currently being recorded.
    pub recording: bool,

    /// Headset associated with this session.
    pub headset: Option<HeadsetInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_session_info() {
        let json = r#"{
            "id": "session-uuid-456",
            "status": "activated",
            "owner": "user123",
            "license": "license-abc",
            "appId": "com.example.app",
            "started": "2024-01-15T10:30:00Z",
            "streams": ["eeg", "dev"],
            "recordIds": [],
            "recording": false
        }"#;

        let session: SessionInfo = serde_json::from_str(json).unwrap();
        assert_eq!(session.id, "session-uuid-456");
        assert_eq!(session.status, "activated");
        assert_eq!(session.owner, "user123");
        assert_eq!(session.license, "license-abc");
        assert_eq!(session.streams, vec!["eeg", "dev"]);
        assert!(!session.recording);
        assert!(session.stopped.is_none());
        assert!(session.headset.is_none());
    }
}
