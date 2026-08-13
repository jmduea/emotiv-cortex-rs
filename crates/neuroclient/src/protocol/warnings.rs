//! Cortex warning objects.
//!
//! Besides RPC responses and stream samples, Cortex sends a third
//! message type: warnings. They notify the application about lifecycle
//! events such as user login/logout, profile changes, headset
//! connectivity, and — critically — automatic cancellation of stream
//! subscriptions (code 0) and sessions (code 1).
//!
//! See <https://emotiv.gitbook.io/cortex-api/warning-objects>.

use serde::Deserialize;

/// Well-known Cortex warning codes.
///
/// Unknown codes are still delivered to warning receivers; this list
/// only names the codes the client itself reacts to or that are common
/// enough to warrant constants.
pub struct WarningCodes;

impl WarningCodes {
    /// All subscribed data streams were automatically canceled by Cortex
    /// (headset disconnected or session closed).
    pub const STREAMS_AUTO_CANCELED: i64 = 0;
    /// The session was automatically closed by Cortex.
    pub const SESSION_AUTO_CLOSED: i64 = 1;
    /// The user logged in via the EMOTIV App.
    pub const USER_LOGIN: i64 = 2;
    /// The user logged out via the EMOTIV App.
    pub const USER_LOGOUT: i64 = 3;
    /// The user approved this application.
    pub const ACCESS_GRANTED: i64 = 9;
    /// The user declined this application.
    pub const ACCESS_DENIED: i64 = 10;
    /// A training profile was loaded.
    pub const PROFILE_LOADED: i64 = 13;
    /// A training profile was unloaded.
    pub const PROFILE_UNLOADED: i64 = 14;
    /// A training profile was automatically unloaded by Cortex.
    pub const PROFILE_AUTO_UNLOADED: i64 = 15;
    /// A headset was connected.
    pub const HEADSET_CONNECTED: i64 = 104;
    /// Cortex disconnected a headset after a data timeout.
    pub const HEADSET_DATA_TIMEOUT: i64 = 103;
}

/// A warning object pushed by Cortex.
///
/// `message` is deliberately kept as raw JSON: for most warnings it is a
/// string, but several codes carry structured objects. Unknown or future
/// codes are preserved as-is (forward compatible).
#[derive(Clone, Deserialize)]
pub struct CortexWarning {
    /// Numeric warning code (see [`WarningCodes`]).
    pub code: i64,

    /// Code-dependent payload: a string for most warnings, an object for
    /// others (e.g. `{ "behavior": ..., "sessionId": ... }`).
    #[serde(default)]
    pub message: serde_json::Value,
}

impl std::fmt::Debug for CortexWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CortexWarning")
            .field("code", &self.code)
            .field("message", &"<redacted>")
            .finish()
    }
}

impl CortexWarning {
    /// The `sessionId` carried by session-scoped warnings (codes 0/1),
    /// if present.
    #[must_use]
    pub fn session_id(&self) -> Option<&str> {
        self.message.get("sessionId").and_then(|v| v.as_str())
    }

    /// Returns `true` if this warning cancels the subscriptions or the
    /// session identified by [`session_id`](Self::session_id).
    #[must_use]
    pub fn cancels_session_streams(&self) -> bool {
        matches!(
            self.code,
            WarningCodes::STREAMS_AUTO_CANCELED | WarningCodes::SESSION_AUTO_CLOSED
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_structured_warning() {
        let json = r#"{
            "code": 0,
            "message": {
                "behavior": "Cortex has stopped all the subscriptions of session abc.",
                "sessionId": "session-abc"
            }
        }"#;

        let warning: CortexWarning = serde_json::from_str(json).unwrap();
        assert_eq!(warning.code, WarningCodes::STREAMS_AUTO_CANCELED);
        assert_eq!(warning.session_id(), Some("session-abc"));
        assert!(warning.cancels_session_streams());
    }

    #[test]
    fn test_deserialize_string_warning() {
        let json = r#"{"code": 2, "message": "User jon.snow has already logged in."}"#;

        let warning: CortexWarning = serde_json::from_str(json).unwrap();
        assert_eq!(warning.code, WarningCodes::USER_LOGIN);
        assert_eq!(warning.session_id(), None);
        assert!(!warning.cancels_session_streams());
    }

    #[test]
    fn test_unknown_code_preserved() {
        let json = r#"{"code": 9999, "message": {"future": true}}"#;

        let warning: CortexWarning = serde_json::from_str(json).unwrap();
        assert_eq!(warning.code, 9999);
        assert!(!warning.cancels_session_streams());
    }

    #[test]
    fn debug_redacts_warning_message() {
        let warning: CortexWarning =
            serde_json::from_str(r#"{"code": 9999, "message": "SENTINEL-WARNING"}"#).unwrap();

        let debug = format!("{warning:?}");
        assert!(!debug.contains("SENTINEL-WARNING"));
        assert!(debug.contains("9999"));
    }
}
