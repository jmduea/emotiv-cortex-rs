//! Headset discovery, metadata, and config-mapping protocol types.

use std::collections::HashMap;

use serde::Deserialize;

/// Options for the `queryHeadsets` method.
#[derive(Debug, Clone, Default)]
pub struct QueryHeadsetsOptions {
    /// Filter by a specific headset id.
    pub id: Option<String>,
    /// Include flex mapping details in the response payload.
    pub include_flex_mappings: bool,
}

/// Headset info returned by `queryHeadsets`.
#[derive(Debug, Clone, Deserialize)]
pub struct HeadsetInfo {
    /// Headset ID (e.g., "INSIGHT-A1B2C3D4").
    pub id: String,

    /// Connection status: "discovered", "connecting", "connected".
    pub status: String,

    /// How the headset is connected: "dongle", "bluetooth", "usb cable".
    #[serde(rename = "connectedBy")]
    pub connected_by: Option<String>,

    /// Dongle serial number, if connected via USB dongle.
    #[serde(rename = "dongle")]
    pub dongle_serial: Option<String>,

    /// Firmware version string.
    pub firmware: Option<String>,

    /// Motion sensor names available on this headset.
    #[serde(rename = "motionSensors")]
    pub motion_sensors: Option<Vec<String>>,

    /// EEG sensor/channel names available on this headset.
    pub sensors: Option<Vec<String>>,

    /// Device-specific settings.
    pub settings: Option<serde_json::Value>,

    /// Mapping of EEG channels to headset sensor locations (EPOC Flex).
    ///
    /// The Cortex docs and payloads have used both `flexMappings` and
    /// `flexMapping` over time; we accept either for compatibility.
    #[serde(rename = "flexMappings", alias = "flexMapping")]
    pub flex_mapping: Option<serde_json::Value>,

    /// Headband position (EPOC X).
    #[serde(rename = "headbandPosition")]
    pub headband_position: Option<String>,

    /// Custom name of the headset, if set by the user.
    #[serde(rename = "customName")]
    pub custom_name: Option<String>,

    /// Virtual headset flag (true for virtual devices)
    #[serde(rename = "isVirtual")]
    pub is_virtual: Option<bool>,

    /// Device mode (for example, "EPOC", "EPOC+", "EPOC X").
    pub mode: Option<String>,

    /// Battery percentage in range [0, 100].
    #[serde(rename = "batteryPercent")]
    pub battery_percent: Option<u32>,

    /// Signal strength indicator.
    #[serde(rename = "signalStrength")]
    pub signal_strength: Option<u32>,

    /// Power status.
    pub power: Option<String>,

    /// ID for virtual headset pairing.
    #[serde(rename = "virtualHeadsetId")]
    pub virtual_headset_id: Option<String>,

    /// User-friendly firmware display string.
    #[serde(rename = "firmwareDisplay")]
    pub firmware_display: Option<String>,

    /// Whether the device is in DFU mode.
    #[serde(rename = "isDfuMode")]
    pub is_dfu_mode: Option<bool>,

    /// Supported DFU update types.
    #[serde(rename = "dfuTypes")]
    pub dfu_types: Option<Vec<String>>,

    /// System uptime value reported by Cortex.
    #[serde(rename = "systemUpTime")]
    pub system_up_time: Option<u64>,

    /// Device uptime value reported by Cortex.
    pub uptime: Option<u64>,

    /// Bluetooth uptime value reported by Cortex.
    #[serde(rename = "bluetoothUpTime")]
    pub bluetooth_up_time: Option<u64>,

    /// Internal device counter value.
    pub counter: Option<u64>,

    /// Forward-compatible storage for new optional fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Result payload from `syncWithHeadsetClock`.
#[derive(Debug, Clone, Deserialize)]
pub struct HeadsetClockSyncResult {
    /// Clock adjustment reported by Cortex.
    pub adjustment: f64,
    /// Headset id associated with the sync result.
    pub headset: String,
}

/// Type of operation requested for `configMapping`.
#[derive(Debug, Clone, Copy)]
pub enum ConfigMappingMode {
    Create,
    Get,
    Read,
    Update,
    Delete,
}

impl ConfigMappingMode {
    /// Returns the Cortex API status string for this mode.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            ConfigMappingMode::Create => "create",
            ConfigMappingMode::Get => "get",
            ConfigMappingMode::Read => "read",
            ConfigMappingMode::Update => "update",
            ConfigMappingMode::Delete => "delete",
        }
    }
}

/// Request variants for `configMapping`.
#[derive(Debug, Clone)]
pub enum ConfigMappingRequest {
    /// Create a new mapping configuration.
    Create {
        name: String,
        mappings: serde_json::Value,
    },
    /// Retrieve all mapping configurations.
    Get,
    /// Read a specific mapping configuration.
    Read { uuid: String },
    /// Update a mapping configuration.
    Update {
        uuid: String,
        name: Option<String>,
        mappings: Option<serde_json::Value>,
    },
    /// Delete a mapping configuration.
    Delete { uuid: String },
}

impl ConfigMappingRequest {
    /// Returns the operation mode for this request.
    #[must_use]
    pub fn mode(&self) -> ConfigMappingMode {
        match self {
            ConfigMappingRequest::Create { .. } => ConfigMappingMode::Create,
            ConfigMappingRequest::Get => ConfigMappingMode::Get,
            ConfigMappingRequest::Read { .. } => ConfigMappingMode::Read,
            ConfigMappingRequest::Update { .. } => ConfigMappingMode::Update,
            ConfigMappingRequest::Delete { .. } => ConfigMappingMode::Delete,
        }
    }
}

/// Mapping object returned for create/read/update operations.
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigMappingValue {
    /// Optional mapping label metadata.
    pub label: Option<serde_json::Value>,
    /// EEG sensor mapping pairs.
    pub mappings: serde_json::Value,
    /// Mapping name.
    pub name: String,
    /// Mapping UUID.
    pub uuid: String,
    /// Forward-compatible storage for additional fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Value payload returned by the `get` mode of `configMapping`.
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigMappingListValue {
    /// Available mapping configurations.
    #[serde(default)]
    pub config: Vec<ConfigMappingValue>,
    /// Last update timestamp.
    pub updated: Option<String>,
    /// Version identifier.
    pub version: Option<String>,
    /// Forward-compatible storage for additional fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Parsed response variants for `configMapping`.
#[derive(Debug, Clone)]
pub enum ConfigMappingResponse {
    /// Response for create/read/update modes.
    Value {
        message: String,
        value: ConfigMappingValue,
    },
    /// Response for get mode.
    List {
        message: String,
        value: ConfigMappingListValue,
    },
    /// Response for delete mode.
    Deleted { message: String, uuid: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_headset_info() {
        let json = r#"{
            "id": "INSIGHT-A1B2C3D4",
            "dongle": "6ff",
            "firmware": "925",
            "status": "connected",
            "connectedBy": "dongle",
            "motionSensors": ["GYROX", "GYROY", "GYROZ", "ACCX", "ACCY", "ACCZ"],
            "sensors": ["AF3", "AF4", "T7", "T8", "Pz"]
        }"#;

        let info: HeadsetInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, "INSIGHT-A1B2C3D4");
        assert_eq!(info.status, "connected");
        assert_eq!(info.sensors.as_ref().unwrap().len(), 5);
        assert!(info.extra.is_empty());
    }

    #[test]
    fn test_deserialize_headset_info_flex_mappings_aliases() {
        let new_json = r#"{
            "id": "EPOCFLEX-ABCD1234",
            "status": "connected",
            "flexMappings": {"AF3":"C1"}
        }"#;
        let old_json = r#"{
            "id": "EPOCFLEX-ABCD1234",
            "status": "connected",
            "flexMapping": {"AF3":"C1"}
        }"#;

        let new_info: HeadsetInfo = serde_json::from_str(new_json).unwrap();
        let old_info: HeadsetInfo = serde_json::from_str(old_json).unwrap();

        assert!(new_info.flex_mapping.is_some());
        assert!(old_info.flex_mapping.is_some());
    }

    #[test]
    fn test_deserialize_headset_info_extended_fields() {
        let json = r#"{
            "id": "EPOCX-11223344",
            "status": "connected",
            "mode": "EPOC X",
            "batteryPercent": 87,
            "signalStrength": 2,
            "power": "on",
            "virtualHeadsetId": "VH-001",
            "firmwareDisplay": "3.7.1",
            "isDfuMode": false,
            "dfuTypes": ["firmware"],
            "systemUpTime": 12345,
            "uptime": 12300,
            "bluetoothUpTime": 12000,
            "counter": 91,
            "futureField": "future"
        }"#;

        let info: HeadsetInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.mode.as_deref(), Some("EPOC X"));
        assert_eq!(info.battery_percent, Some(87));
        assert_eq!(info.signal_strength, Some(2));
        assert_eq!(info.power.as_deref(), Some("on"));
        assert_eq!(info.virtual_headset_id.as_deref(), Some("VH-001"));
        assert_eq!(info.firmware_display.as_deref(), Some("3.7.1"));
        assert_eq!(info.is_dfu_mode, Some(false));
        assert_eq!(info.dfu_types, Some(vec!["firmware".to_string()]));
        assert_eq!(info.system_up_time, Some(12345));
        assert_eq!(info.uptime, Some(12300));
        assert_eq!(info.bluetooth_up_time, Some(12000));
        assert_eq!(info.counter, Some(91));
        assert_eq!(
            info.extra.get("futureField"),
            Some(&serde_json::json!("future"))
        );
    }

    #[test]
    fn test_deserialize_headset_clock_sync_result() {
        let json = r#"{
            "adjustment": 0.0123,
            "headset": "INSIGHT-A1B2C3D4"
        }"#;

        let sync: HeadsetClockSyncResult = serde_json::from_str(json).unwrap();
        assert!((sync.adjustment - 0.0123).abs() < f64::EPSILON);
        assert_eq!(sync.headset, "INSIGHT-A1B2C3D4");
    }
    #[test]
    fn test_deserialize_config_mapping_value_response_shapes() {
        let value_json = r#"{
            "message": "Create flex mapping config successful",
            "value": {
                "label": {},
                "mappings": {"CMS":"TP9"},
                "name": "config1",
                "uuid": "4416dc1b-3a7c-4d20-9ec6-aacdb9930071"
            }
        }"#;
        let list_json = r#"{
            "message": "Get flex mapping config successful",
            "value": {
                "config": [{
                    "label": {},
                    "mappings": {"CMS":"TP10"},
                    "name": "Default Configuration",
                    "uuid": "f4296b2d-d6e7-45cf-9569-7bc2a1bd56e4"
                }],
                "updated": "2025-10-08T06:16:30.521+07:00",
                "version": "2018-05-08"
            }
        }"#;
        let delete_json = r#"{
            "message": "Delete flex mapping config successful",
            "uuid": "effa621f-49d6-4c46-95f3-28f43813a6e9"
        }"#;

        #[derive(serde::Deserialize)]
        struct ValueEnvelope {
            message: String,
            value: ConfigMappingValue,
        }
        #[derive(serde::Deserialize)]
        struct ListEnvelope {
            message: String,
            value: ConfigMappingListValue,
        }
        #[derive(serde::Deserialize)]
        struct DeleteEnvelope {
            message: String,
            uuid: String,
        }

        let value: ValueEnvelope = serde_json::from_str(value_json).unwrap();
        assert_eq!(value.message, "Create flex mapping config successful");
        assert_eq!(value.value.name, "config1");

        let list: ListEnvelope = serde_json::from_str(list_json).unwrap();
        assert_eq!(list.message, "Get flex mapping config successful");
        assert_eq!(list.value.config.len(), 1);
        assert_eq!(list.value.version.as_deref(), Some("2018-05-08"));

        let deleted: DeleteEnvelope = serde_json::from_str(delete_json).unwrap();
        assert_eq!(deleted.message, "Delete flex mapping config successful");
        assert_eq!(deleted.uuid, "effa621f-49d6-4c46-95f3-28f43813a6e9");
    }
}
