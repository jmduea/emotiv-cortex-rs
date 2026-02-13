//! Authentication-related protocol types.

use serde::Deserialize;

/// User login info from `getUserLogin`.
#[derive(Debug, Clone, Deserialize)]
pub struct UserLoginInfo {
    /// Logged-in username.
    pub username: String,
    /// User's current login provider.
    #[serde(rename = "currentOSUId")]
    pub current_os_uid: Option<String>,
    /// Login time.
    #[serde(rename = "lastLoginTime")]
    pub last_login_time: Option<String>,
}
