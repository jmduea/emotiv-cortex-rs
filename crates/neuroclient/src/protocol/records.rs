//! Record and marker protocol types.

use serde::{Deserialize, Serialize};

/// Record information from `createRecord` / `queryRecords`.
#[derive(Debug, Clone, Deserialize)]
pub struct RecordInfo {
    /// Record UUID.
    pub uuid: String,

    /// Record title.
    pub title: Option<String>,

    /// Start time (ISO 8601).
    #[serde(rename = "startDatetime")]
    pub start_datetime: Option<String>,

    /// End time (ISO 8601), `None` if still recording.
    #[serde(rename = "endDatetime")]
    pub end_datetime: Option<String>,
}

/// Marker information from `injectMarker`.
#[derive(Debug, Clone, Deserialize)]
pub struct MarkerInfo {
    /// Marker UUID.
    pub uuid: String,

    /// Marker start time (ISO 8601).
    #[serde(rename = "startDatetime")]
    pub start_datetime: Option<String>,
}

/// Export format for `exportRecord`.
#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    /// Comma-separated values.
    Csv,
    /// European Data Format (standard for EEG recordings).
    Edf,
}

impl ExportFormat {
    /// Returns the Cortex API string for this format.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            ExportFormat::Csv => "CSV",
            ExportFormat::Edf => "EDF",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_record_info() {
        let json = r#"{
            "uuid": "record-uuid-789",
            "title": "Calibration Session 1",
            "startDatetime": "2024-01-15T10:30:00Z"
        }"#;

        let record: RecordInfo = serde_json::from_str(json).unwrap();
        assert_eq!(record.uuid, "record-uuid-789");
        assert_eq!(record.title.as_deref(), Some("Calibration Session 1"));
        assert!(record.end_datetime.is_none());
    }

    #[test]
    fn test_deserialize_marker_info() {
        let json = r#"{
            "uuid": "marker-uuid-abc",
            "startDatetime": "2024-01-15T10:30:05Z"
        }"#;

        let marker: MarkerInfo = serde_json::from_str(json).unwrap();
        assert_eq!(marker.uuid, "marker-uuid-abc");
    }
    #[test]
    fn test_export_format_strings() {
        assert_eq!(ExportFormat::Csv.as_str(), "CSV");
        assert_eq!(ExportFormat::Edf.as_str(), "EDF");
    }
}

/// Request payload for `updateRecord`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRecordRequest {
    /// Record UUID.
    pub record_id: String,
    /// Optional new title.
    pub title: Option<String>,
    /// Optional new description.
    pub description: Option<String>,
    /// Optional replacement tag list.
    pub tags: Option<Vec<String>>,
}

impl UpdateRecordRequest {
    /// Create a minimal update request targeting a specific record ID.
    pub fn new(record_id: impl Into<String>) -> Self {
        Self {
            record_id: record_id.into(),
            title: None,
            description: None,
            tags: None,
        }
    }
}
