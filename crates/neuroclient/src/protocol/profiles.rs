//! Profile protocol types.

use std::collections::HashMap;

use serde::Deserialize;

/// Profile information from `queryProfile`.
#[derive(Debug, Clone, Deserialize)]
pub struct ProfileInfo {
    /// Profile UUID.
    pub uuid: String,

    /// Profile name.
    pub name: String,

    /// Whether the profile is read-only.
    #[serde(rename = "readOnly")]
    pub read_only: bool,

    /// EEG channel list associated with this profile.
    #[serde(rename = "eegChannels")]
    pub eeg_channels: Vec<String>,
}

/// Profile state returned by `getCurrentProfile`.
#[derive(Debug, Clone, Deserialize)]
pub struct CurrentProfileInfo {
    /// Name of the currently loaded profile, or `None` when no profile is loaded.
    pub name: Option<String>,
    /// Whether the profile is loaded by this app.
    #[serde(default, rename = "loadedByThisApp")]
    pub loaded_by_this_app: bool,
    /// Forward-compatible storage for additional fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Actions for the `setupProfile` method.
#[derive(Debug, Clone, Copy)]
pub enum ProfileAction {
    /// Create a new empty profile.
    Create,
    /// Load a profile for a headset.
    Load,
    /// Unload a profile from a headset.
    Unload,
    /// Save the current profile state.
    Save,
    /// Rename a profile.
    Rename,
    /// Delete a profile.
    Delete,
}

impl ProfileAction {
    /// Returns the Cortex API string for this action.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            ProfileAction::Create => "create",
            ProfileAction::Load => "load",
            ProfileAction::Unload => "unload",
            ProfileAction::Save => "save",
            ProfileAction::Rename => "rename",
            ProfileAction::Delete => "delete",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_profile_info() {
        let json = r#"{
            "uuid": "profile-uuid-123",
            "name": "my_profile",
            "readOnly": false,
            "eegChannels": ["AF3", "AF4"]
        }"#;

        let profile: ProfileInfo = serde_json::from_str(json).unwrap();
        assert_eq!(profile.uuid, "profile-uuid-123");
        assert_eq!(profile.name, "my_profile");
        assert!(!profile.read_only);
        assert_eq!(
            profile.eeg_channels,
            vec!["AF3".to_string(), "AF4".to_string()]
        );
    }

    #[test]
    fn test_deserialize_current_profile_info() {
        let json = r#"{
            "name": "my_profile",
            "loadedByThisApp": true
        }"#;

        let profile: CurrentProfileInfo = serde_json::from_str(json).unwrap();
        assert_eq!(profile.name.as_deref(), Some("my_profile"));
        assert!(profile.loaded_by_this_app);
        assert!(profile.extra.is_empty());
    }

    #[test]
    fn test_deserialize_current_profile_info_null_name_and_default_bool() {
        let json = r#"{
            "name": null,
            "extraField": "x"
        }"#;

        let profile: CurrentProfileInfo = serde_json::from_str(json).unwrap();
        assert!(profile.name.is_none());
        assert!(!profile.loaded_by_this_app);
        assert_eq!(
            profile.extra.get("extraField"),
            Some(&serde_json::json!("x"))
        );
    }
    #[test]
    fn test_profile_action_strings() {
        assert_eq!(ProfileAction::Create.as_str(), "create");
        assert_eq!(ProfileAction::Load.as_str(), "load");
        assert_eq!(ProfileAction::Unload.as_str(), "unload");
        assert_eq!(ProfileAction::Save.as_str(), "save");
        assert_eq!(ProfileAction::Rename.as_str(), "rename");
        assert_eq!(ProfileAction::Delete.as_str(), "delete");
    }
}
