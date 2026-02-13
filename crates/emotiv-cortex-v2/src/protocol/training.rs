//! Training and advanced BCI protocol types.

use serde::{Deserialize, Serialize};

/// Detection type for the `training` and `getDetectionInfo` methods.
#[derive(Debug, Clone, Copy)]
pub enum DetectionType {
    /// Mental command detection.
    MentalCommand,
    /// Facial expression detection.
    FacialExpression,
}

impl DetectionType {
    /// Returns the Cortex API string for this detection type.
    pub fn as_str(&self) -> &'static str {
        match self {
            DetectionType::MentalCommand => "mentalCommand",
            DetectionType::FacialExpression => "facialExpression",
        }
    }
}

/// Training status/command for the `training` method.
#[derive(Debug, Clone, Copy)]
pub enum TrainingStatus {
    /// Start a new training for the specified action.
    Start,
    /// Accept a successful training and add it to the profile.
    Accept,
    /// Reject a completed training without saving.
    Reject,
    /// Cancel the current training session.
    Reset,
    /// Erase all training data for the specified action.
    Erase,
}

impl TrainingStatus {
    /// Returns the Cortex API string for this status.
    pub fn as_str(&self) -> &'static str {
        match self {
            TrainingStatus::Start => "start",
            TrainingStatus::Accept => "accept",
            TrainingStatus::Reject => "reject",
            TrainingStatus::Reset => "reset",
            TrainingStatus::Erase => "erase",
        }
    }
}

/// Detection info from `getDetectionInfo`.
#[derive(Debug, Clone, Deserialize)]
pub struct DetectionInfo {
    /// Available actions for this detection type.
    pub actions: Vec<String>,
    /// Available training controls.
    pub controls: Vec<String>,
    /// Possible training events.
    pub events: Vec<String>,
}
/// Trained signature actions from `getTrainedSignatureActions`.
#[derive(Debug, Clone, Deserialize)]
pub struct TrainedSignatureActions {
    /// Total number of training sessions performed.
    #[serde(rename = "totalTimesTraining")]
    pub total_times_training: u32,

    /// Per-action training counts.
    #[serde(rename = "trainedActions")]
    pub trained_actions: Vec<TrainedAction>,
}

/// A single trained action within a profile.
#[derive(Debug, Clone, Deserialize)]
pub struct TrainedAction {
    /// Action name (e.g. "neutral", "push", "pull").
    pub action: String,
    /// Number of times this action has been trained.
    pub times: u32,
}

/// Training time info from `getTrainingTime`.
#[derive(Debug, Clone, Deserialize)]
pub struct TrainingTime {
    /// Training duration in seconds.
    pub time: f64,
}

/// Request payload for `mentalCommandTrainingThreshold`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MentalCommandTrainingThresholdRequest {
    /// Session ID target. Mutually exclusive with `profile`.
    pub session_id: Option<String>,
    /// Profile name target. Mutually exclusive with `session_id`.
    pub profile: Option<String>,
    /// Explicit status (`"get"` / `"set"`). When omitted, inferred from `value`.
    pub status: Option<String>,
    /// Threshold value used with `status = "set"`.
    pub value: Option<f64>,
}

/// Request payload for `facialExpressionSignatureType`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacialExpressionSignatureTypeRequest {
    /// Operation status (`"get"` / `"set"`).
    pub status: String,
    /// Profile target (optional).
    pub profile: Option<String>,
    /// Session target (optional).
    pub session: Option<String>,
    /// Signature value for set operations.
    pub signature: Option<String>,
}

/// Request payload for `facialExpressionThreshold`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacialExpressionThresholdRequest {
    /// Operation status (`"get"` / `"set"`).
    pub status: String,
    /// Facial action name.
    pub action: String,
    /// Profile target (optional).
    pub profile: Option<String>,
    /// Session target (optional).
    pub session: Option<String>,
    /// Threshold value for set operations.
    pub value: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detection_type_strings() {
        assert_eq!(DetectionType::MentalCommand.as_str(), "mentalCommand");
        assert_eq!(DetectionType::FacialExpression.as_str(), "facialExpression");
    }

    #[test]
    fn test_training_status_strings() {
        assert_eq!(TrainingStatus::Start.as_str(), "start");
        assert_eq!(TrainingStatus::Accept.as_str(), "accept");
        assert_eq!(TrainingStatus::Reject.as_str(), "reject");
        assert_eq!(TrainingStatus::Reset.as_str(), "reset");
        assert_eq!(TrainingStatus::Erase.as_str(), "erase");
    }
    #[test]
    fn test_deserialize_trained_signature_actions() {
        let json = r#"{
            "totalTimesTraining": 15,
            "trainedActions": [
                {"action": "neutral", "times": 8},
                {"action": "push", "times": 4},
                {"action": "pull", "times": 3}
            ]
        }"#;

        let actions: TrainedSignatureActions = serde_json::from_str(json).unwrap();
        assert_eq!(actions.total_times_training, 15);
        assert_eq!(actions.trained_actions.len(), 3);
        assert_eq!(actions.trained_actions[0].action, "neutral");
        assert_eq!(actions.trained_actions[0].times, 8);
        assert_eq!(actions.trained_actions[2].action, "pull");
    }

    #[test]
    fn test_deserialize_training_time() {
        let json = r#"{"time": 8.0}"#;

        let tt: TrainingTime = serde_json::from_str(json).unwrap();
        assert!((tt.time - 8.0).abs() < f64::EPSILON);
    }
}
