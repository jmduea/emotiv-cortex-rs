//! Subject and demographic protocol types.

use serde::{Deserialize, Serialize};

/// Subject info from `createSubject` / `updateSubject` / `querySubjects`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectInfo {
    /// Subject name (unique identifier within a user's account).
    #[serde(rename = "subjectName")]
    pub subject_name: String,

    /// Date of birth (ISO 8601 date, e.g. "1990-01-15").
    #[serde(rename = "dateOfBirth")]
    pub date_of_birth: Option<String>,

    /// Biological sex: "M", "F", or "U" (unknown).
    pub sex: Option<String>,

    /// ISO 3166-1 alpha-2 country code (e.g. "US", "GB").
    #[serde(rename = "countryCode")]
    pub country_code: Option<String>,

    /// Full country name.
    #[serde(rename = "countryName")]
    pub country_name: Option<String>,

    /// State or province.
    pub state: Option<String>,

    /// City name.
    pub city: Option<String>,

    /// Custom demographic attributes as key-value pairs.
    pub attributes: Option<Vec<serde_json::Value>>,

    /// Number of experiments/recordings associated with this subject.
    #[serde(rename = "experimentsCount")]
    pub experiments_count: Option<u32>,
}

/// A demographic attribute type from `getDemographicAttributes`.
///
/// Each attribute has a name and a list of valid values.
#[derive(Debug, Clone, Deserialize)]
pub struct DemographicAttribute {
    /// Attribute name (e.g. "sex", "country").
    pub name: String,
    /// Valid values for this attribute.
    pub value: Vec<String>,
}

/// Request payload for subject create/update operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectRequest {
    /// Subject name (unique identifier within a user's account).
    pub subject_name: String,
    /// Date of birth (ISO 8601 date, e.g. "1990-01-15").
    pub date_of_birth: Option<String>,
    /// Biological sex: "M", "F", or "U" (unknown).
    pub sex: Option<String>,
    /// ISO 3166-1 alpha-2 country code (e.g. "US", "GB").
    pub country_code: Option<String>,
    /// State or province.
    pub state: Option<String>,
    /// City name.
    pub city: Option<String>,
    /// Custom demographic attributes as key-value pairs.
    pub attributes: Option<Vec<serde_json::Value>>,
}

impl SubjectRequest {
    /// Create a minimal subject request with just the subject name.
    pub fn new(subject_name: impl Into<String>) -> Self {
        Self {
            subject_name: subject_name.into(),
            date_of_birth: None,
            sex: None,
            country_code: None,
            state: None,
            city: None,
            attributes: None,
        }
    }
}

/// Request payload for `querySubjects`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuerySubjectsRequest {
    /// Query expression object.
    pub query: serde_json::Value,
    /// Sort order expression array/object.
    pub order_by: serde_json::Value,
    /// Optional pagination limit.
    pub limit: Option<u32>,
    /// Optional pagination offset.
    pub offset: Option<u32>,
}

impl Default for QuerySubjectsRequest {
    fn default() -> Self {
        Self {
            query: serde_json::json!({}),
            order_by: serde_json::json!([]),
            limit: None,
            offset: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_subject_info() {
        let json = r#"{
            "subjectName": "subject01",
            "dateOfBirth": "1990-01-15",
            "sex": "M",
            "countryCode": "US",
            "countryName": "United States",
            "state": "California",
            "city": "San Francisco",
            "experimentsCount": 5
        }"#;

        let info: SubjectInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.subject_name, "subject01");
        assert_eq!(info.date_of_birth.as_deref(), Some("1990-01-15"));
        assert_eq!(info.sex.as_deref(), Some("M"));
        assert_eq!(info.country_code.as_deref(), Some("US"));
        assert_eq!(info.experiments_count, Some(5));
        assert!(info.attributes.is_none());
    }

    #[test]
    fn test_deserialize_subject_info_minimal() {
        let json = r#"{"subjectName": "subject02"}"#;

        let info: SubjectInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.subject_name, "subject02");
        assert!(info.date_of_birth.is_none());
        assert!(info.sex.is_none());
        assert!(info.experiments_count.is_none());
    }

    #[test]
    fn test_deserialize_demographic_attribute() {
        let json = r#"[
            {"name": "sex", "value": ["M", "F", "U"]},
            {"name": "country", "value": ["US", "GB", "DE"]}
        ]"#;

        let attrs: Vec<DemographicAttribute> = serde_json::from_str(json).unwrap();
        assert_eq!(attrs.len(), 2);
        assert_eq!(attrs[0].name, "sex");
        assert_eq!(attrs[0].value, vec!["M", "F", "U"]);
        assert_eq!(attrs[1].name, "country");
    }
}
