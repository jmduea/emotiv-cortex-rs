//! # Cortex API JSON-RPC 2.0 Protocol Types
//!
//! Data structures for communicating with the Emotiv Cortex API.
//! The Cortex API uses WebSocket + JSON-RPC 2.0 at `wss://localhost:6868`.
//!
//! ## API Flow
//!
//! ```text
//! connect(wss://localhost:6868)
//!   → requestAccess(clientId, clientSecret)
//!   → authorize(clientId, clientSecret)        → cortexToken
//!   → queryHeadsets()                           → Vec<HeadsetInfo>
//!   → createSession(cortexToken, headsetId)     → SessionInfo
//!   → subscribe(cortexToken, sessionId, [...])  → data streams
//! ```
//!
//! ## Available Data Streams
//!
//! | Stream | Description | API Level |
//! |--------|-------------|-----------|
//! | `eeg`  | Raw EEG channel data | Premium, (basic for consumer devices) |
//! | `dev`  | Battery, signal, contact quality | Basic |
//! | `mot`  | Accelerometer, magnetometer, gyroscope/quaternion | Basic |
//! | `eq`   | Per-sensor EEG quality metrics | Basic |
//! | `pow`  | Band power (theta/alpha/betaL/betaH/gamma) | Basic |
//! | `met`  | Performance metrics (attention, stress, etc.) | Basic (0.1Hz) / Premium (2Hz) |
//! | `com`  | Mental command action + power | Basic |
//! | `fac`  | Facial expressions | Basic |
//! | `sys`  | System/training events | Basic |

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ─── JSON-RPC Protocol ──────────────────────────────────────────────────

/// A JSON-RPC 2.0 request to the Cortex API.
#[derive(Debug, Serialize)]
pub struct CortexRequest {
    pub id: u64,
    pub jsonrpc: &'static str,
    pub method: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    // Using `serde_json::Value` allows us to flexibly construct params for different methods without needing a separate struct for each method's parameters.
    pub params: Option<serde_json::Value>,
}

impl CortexRequest {
    /// Create a new request with the given method and params.
    pub fn new(id: u64, method: &'static str, params: serde_json::Value) -> Self {
        let params = if params.is_object() && params.as_object().is_some_and(|m| m.is_empty()) {
            None
        } else {
            Some(params)
        };

        Self {
            jsonrpc: "2.0",
            id,
            method,
            params,
        }
    }
}

/// A JSON-RPC 2.0 response from the Cortex API.
#[derive(Debug, Deserialize)]
pub struct CortexResponse {
    pub id: Option<u64>,
    pub result: Option<serde_json::Value>,
    pub error: Option<RpcError>,
}

/// A JSON-RPC 2.0 error payload from the Cortex API.
///
/// This is the raw error object from the protocol. Use
/// [`CortexError::from_api_error`](crate::CortexError::from_api_error)
/// to convert to a semantic error type.
#[derive(Debug, Clone, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cortex API error {}: {}", self.code, self.message)
    }
}

// ─── Headset & Session ──────────────────────────────────────────────────

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
    Deleted {
        message: String,
        uuid: String,
    },
}

/// Session information from `createSession` / `querySessions`.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionInfo {
    /// Session ID (UUID).
    pub id: String,

    /// Session status: "opened", "activated".
    pub status: String,

    /// EmotivID of the user
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

// ─── Data Stream Events ─────────────────────────────────────────────────

/// An EEG data event from a subscribed stream.
///
/// The `eeg` array is a heterogeneous list whose columns are reported by
/// the `subscribe` response `cols` field. For Emotiv Insight the layout is:
///
/// `[COUNTER, INTERPOLATED, AF3, T7, Pz, T8, AF4, RAW_CQ, MARKER_HARDWARE, MARKERS]`
///
/// The trailing `MARKERS` element is an array (often `[]`), so the field
/// is typed as `Vec<serde_json::Value>`. Use [`EegData::from_eeg_array`]
/// to extract strongly-typed channel data.
#[derive(Debug, Deserialize)]
pub struct EegEvent {
    /// Session ID.
    pub sid: String,

    /// Timestamp (Unix seconds as f64, from Cortex).
    pub time: f64,

    /// Raw EEG values including counter, interpolation flag, channels,
    /// contact quality, and markers.
    pub eeg: Vec<serde_json::Value>,
}

/// Parsed EEG channel data from an `"eeg"` stream event.
///
/// Produced by [`EegData::from_eeg_array`], which mirrors the pattern
/// used by [`DeviceQuality::from_dev_array`] for the `"dev"` stream.
#[derive(Debug, Clone)]
pub struct EegData {
    /// Timestamp in microseconds (converted from Cortex f64 seconds).
    pub timestamp: i64,
    /// Sample counter from the headset (wraps at device-specific max).
    pub counter: u32,
    /// Whether this sample was interpolated (`true`) or measured (`false`).
    pub interpolated: bool,
    /// EEG channel values in microvolts, ordered by headset channel layout.
    ///
    /// Insight: `[AF3, T7, Pz, T8, AF4]` (5 channels)
    /// EPOC:    `[AF3, F7, F3, FC5, T7, P7, O1, O2, P8, T8, FC6, F4, F8, AF4]` (14 channels)
    pub channels: Vec<f32>,
    /// Raw contact quality value (0 = off head, higher = better).
    pub raw_cq: f32,
}

impl EegData {
    /// Parse an [`EegEvent::eeg`] array into structured EEG data.
    ///
    /// Expected layout:
    /// `[COUNTER, INTERPOLATED, ch1, .., chN, RAW_CQ, MARKER_HARDWARE, MARKERS]`
    ///
    /// Returns `None` if the array is too short or contains unexpected types.
    pub fn from_eeg_array(
        eeg: &[serde_json::Value],
        num_channels: usize,
        timestamp: f64,
    ) -> Option<Self> {
        // COUNTER + INTERPOLATED + channels + RAW_CQ + MARKER_HARDWARE + MARKERS
        if eeg.len() < 2 + num_channels + 3 {
            return None;
        }

        let counter = eeg[0].as_u64()? as u32;
        let interpolated = eeg[1].as_u64()? != 0;

        let channels: Vec<f32> = eeg[2..2 + num_channels]
            .iter()
            .map(|v| v.as_f64().map(|f| f as f32))
            .collect::<Option<Vec<f32>>>()?;

        let raw_cq = eeg[2 + num_channels].as_f64()? as f32;

        Some(Self {
            timestamp: (timestamp * 1_000_000.0) as i64,
            counter,
            interpolated,
            channels,
            raw_cq,
        })
    }
}

/// A device info event from the "dev" stream.
///
/// Provides battery level, signal strength, and per-channel contact quality.
/// The `dev` array is heterogeneous: `[battery, signal, ch1_cq, ch2_cq, ..., overall_cq, battery_pct]`.
#[derive(Debug, Deserialize)]
pub struct DevEvent {
    /// Session ID.
    pub sid: String,

    /// Timestamp (Unix seconds as f64, from Cortex).
    pub time: f64,

    /// Device status data. Heterogeneous array — see [`DeviceQuality`] for parsed form.
    pub dev: Vec<serde_json::Value>,
}

/// Parsed device quality data from a "dev" stream event.
///
/// Cortex reports contact quality per-channel as integers 0–4 (None/Poor/Fair/Good/Excellent)
/// and overall quality as 0–100. We normalize these to 0.0–1.0 for consistency.
#[derive(Debug, Clone)]
pub struct DeviceQuality {
    /// Battery level 0–4 (coarse indicator).
    pub battery_level: u8,
    /// Wireless signal strength 0.0–1.0.
    pub signal_strength: f32,
    /// Per-channel contact quality, normalized 0.0–1.0 (from Cortex's 0–4 scale).
    pub channel_quality: Vec<f32>,
    /// Overall EEG quality 0.0–1.0 (from Cortex's 0–100 scale).
    pub overall_quality: f32,
    /// Precise battery percentage 0–100.
    pub battery_percent: u8,
}

impl DeviceQuality {
    /// Parse a `DevEvent.dev` array into structured quality data.
    ///
    /// The array format varies by headset:
    /// - Insight (5ch): `[battery, signal, AF3_cq, AF4_cq, T7_cq, T8_cq, Pz_cq, overall, battery_pct]`
    /// - EPOC (14ch): `[battery, signal, AF3_cq, ..., AF4_cq, overall, battery_pct]`
    ///
    /// The `num_channels` parameter tells us how many CQ values to extract.
    pub fn from_dev_array(dev: &[serde_json::Value], num_channels: usize) -> Option<Self> {
        // Minimum: battery + signal + num_channels CQ values + overall + battery_pct
        if dev.len() < 2 + num_channels + 2 {
            return None;
        }

        let battery_level = dev[0].as_u64()? as u8;
        let signal_strength = dev[1].as_f64()? as f32;

        let channel_quality: Vec<f32> = dev[2..2 + num_channels]
            .iter()
            .filter_map(|v| v.as_f64())
            .map(|cq| (cq / 4.0) as f32) // Normalize 0–4 to 0.0–1.0
            .collect();

        if channel_quality.len() != num_channels {
            return None;
        }

        let overall_idx = 2 + num_channels;
        let battery_pct_idx = overall_idx + 1;

        let overall_quality = (dev.get(overall_idx)?.as_f64()? / 100.0) as f32;
        let battery_percent = dev.get(battery_pct_idx)?.as_u64()? as u8;

        Some(Self {
            battery_level,
            signal_strength,
            channel_quality,
            overall_quality,
            battery_percent,
        })
    }
}

/// A motion data event from the "mot" stream.
///
/// Contains accelerometer, magnetometer, and gyroscope or quaternion data.
/// Newer headsets provide quaternions (Q0-Q3), older ones provide gyroscope (GYROX/Y/Z).
///
/// Insight: `[COUNTER_MEMS, INTERPOLATED_MEMS, Q0, Q1, Q2, Q3, ACCX, ACCY, ACCZ, MAGX, MAGY, MAGZ]`
#[derive(Debug, Deserialize)]
pub struct MotEvent {
    /// Session ID.
    pub sid: String,

    /// Timestamp (Unix seconds as f64, from Cortex).
    pub time: f64,

    /// Motion sensor values. Layout varies by headset model.
    pub mot: Vec<f64>,
}

/// Parsed motion/IMU data from a "mot" stream event.
#[derive(Debug, Clone)]
pub struct MotionData {
    /// Timestamp in microseconds.
    pub timestamp: i64,
    /// Quaternion orientation [Q0, Q1, Q2, Q3] (newer headsets).
    pub quaternion: Option<[f32; 4]>,
    /// Accelerometer readings [x, y, z] in g.
    pub accelerometer: [f32; 3],
    /// Magnetometer readings [x, y, z] in microtesla.
    pub magnetometer: [f32; 3],
}

impl MotionData {
    /// Parse a `MotEvent.mot` array into structured motion data.
    ///
    /// Expected format (Insight/EPOC X):
    /// `[COUNTER, INTERPOLATED, Q0, Q1, Q2, Q3, ACCX, ACCY, ACCZ, MAGX, MAGY, MAGZ]`
    pub fn from_mot_array(mot: &[f64], timestamp: f64) -> Option<Self> {
        if mot.len() < 12 {
            return None;
        }

        // Skip COUNTER (0) and INTERPOLATED (1), then Q0-Q3, then ACC, then MAG
        Some(Self {
            timestamp: (timestamp * 1_000_000.0) as i64,
            quaternion: Some([mot[2] as f32, mot[3] as f32, mot[4] as f32, mot[5] as f32]),
            accelerometer: [mot[6] as f32, mot[7] as f32, mot[8] as f32],
            magnetometer: [mot[9] as f32, mot[10] as f32, mot[11] as f32],
        })
    }
}

/// An EEG quality event from the "eq" stream.
///
/// Provides per-sensor signal quality at higher granularity than the "dev" stream.
#[derive(Debug, Deserialize)]
pub struct EqEvent {
    /// Session ID.
    pub sid: String,

    /// Timestamp (Unix seconds as f64, from Cortex).
    pub time: f64,

    /// EEG quality values. Format varies by headset.
    pub eq: Vec<serde_json::Value>,
}

/// Parsed EEG quality data from an "eq" stream event.
#[derive(Debug, Clone)]
pub struct EegQuality {
    /// Battery percentage 0–100.
    pub battery_percent: u8,
    /// Overall EEG quality 0.0–1.0 (normalized from 0–100).
    pub overall: f32,
    /// Sample rate quality 0.0–1.0 (-1 indicates severe packet loss).
    pub sample_rate_quality: f32,
    /// Per-sensor quality 0.0–1.0 (normalized from 0–4).
    pub sensor_quality: Vec<f32>,
}

impl EegQuality {
    /// Parse an `EqEvent.eq` array into structured EEG quality data.
    ///
    /// The array format: `[battery, overall, sr_quality, ch1_q, ch2_q, ..., chN_q]`
    /// where quality values are 0–4 (normalized to 0.0–1.0).
    pub fn from_eq_array(eq: &[serde_json::Value], num_channels: usize) -> Option<Self> {
        // Minimum: battery + overall + sr_quality + num_channels sensor values
        if eq.len() < 3 + num_channels {
            return None;
        }

        let battery_percent = eq[0].as_u64()? as u8;
        let overall = (eq[1].as_f64()? / 100.0) as f32;
        let sample_rate_quality = eq[2].as_f64()? as f32;

        let sensor_quality: Vec<f32> = eq[3..3 + num_channels]
            .iter()
            .filter_map(|v| v.as_f64())
            .map(|q| (q / 4.0) as f32) // Normalize 0–4 to 0.0–1.0
            .collect();

        if sensor_quality.len() != num_channels {
            return None;
        }

        Some(Self {
            battery_percent,
            overall,
            sample_rate_quality,
            sensor_quality,
        })
    }
}

/// A band power event from the "pow" stream.
///
/// Contains frequency band power values per channel. Each channel has 5 bands:
/// theta (4-8Hz), alpha (8-12Hz), betaL (12-16Hz), betaH (16-25Hz), gamma (25-45Hz).
/// Values are absolute power in uV²/Hz.
#[derive(Debug, Deserialize)]
pub struct PowEvent {
    /// Session ID.
    pub sid: String,

    /// Timestamp (Unix seconds as f64, from Cortex).
    pub time: f64,

    /// Band power values: [ch1_theta, ch1_alpha, ch1_betaL, ch1_betaH, ch1_gamma, ch2_theta, ...].
    pub pow: Vec<f64>,
}

/// Parsed band power data from a "pow" stream event.
#[derive(Debug, Clone)]
pub struct BandPowerData {
    /// Timestamp in microseconds.
    pub timestamp: i64,
    /// Per-channel band powers: `[channel][band]` where bands are
    /// `[theta, alpha, betaL, betaH, gamma]` in uV²/Hz.
    pub channel_powers: Vec<[f32; 5]>,
}

impl BandPowerData {
    /// Parse a `PowEvent.pow` array into structured band power data.
    ///
    /// The array is flat: 5 values per channel (theta, alpha, betaL, betaH, gamma).
    pub fn from_pow_array(pow: &[f64], num_channels: usize, timestamp: f64) -> Option<Self> {
        if pow.len() < num_channels * 5 {
            return None;
        }

        let channel_powers: Vec<[f32; 5]> = pow
            .chunks_exact(5)
            .take(num_channels)
            .map(|chunk| {
                [
                    chunk[0] as f32,
                    chunk[1] as f32,
                    chunk[2] as f32,
                    chunk[3] as f32,
                    chunk[4] as f32,
                ]
            })
            .collect();

        Some(Self {
            timestamp: (timestamp * 1_000_000.0) as i64,
            channel_powers,
        })
    }
}

/// A performance metrics event from the "met" stream.
///
/// Provides Emotiv's computed cognitive/emotional state metrics.
/// EPOC/Insight metrics: engagement, excitement, long-term excitement,
/// stress, relaxation, interest, attention, focus.
/// Values are 0.0–1.0 or null if signal quality is insufficient.
#[derive(Debug, Deserialize)]
pub struct MetEvent {
    /// Session ID.
    pub sid: String,

    /// Timestamp (Unix seconds as f64, from Cortex).
    pub time: f64,

    /// Performance metric values.
    pub met: Vec<serde_json::Value>,
}

/// Parsed performance metrics from a "met" stream event.
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Timestamp in microseconds.
    pub timestamp: i64,
    /// Engagement / immersion (0.0–1.0).
    pub engagement: Option<f32>,
    /// Short-term excitement (0.0–1.0).
    pub excitement: Option<f32>,
    /// Long-term excitement, ~1 minute window (0.0–1.0).
    pub long_excitement: Option<f32>,
    /// Stress / emotional tension (0.0–1.0).
    pub stress: Option<f32>,
    /// Relaxation / calm focus (0.0–1.0).
    pub relaxation: Option<f32>,
    /// Interest / attraction-aversion (0.0–1.0).
    pub interest: Option<f32>,
    /// Attention / task focus (0.0–1.0).
    pub attention: Option<f32>,
    /// Focus sustainability (0.0–1.0).
    pub focus: Option<f32>,
}

/// A mental command event from the "com" stream.
///
/// Requires a loaded profile with trained mental commands.
#[derive(Debug, Deserialize)]
pub struct ComEvent {
    /// Session ID.
    pub sid: String,

    /// Timestamp (Unix seconds as f64, from Cortex).
    pub time: f64,

    /// Mental command data: `[action_name, power]`.
    pub com: Vec<serde_json::Value>,
}

/// Parsed mental command data from a "com" stream event.
#[derive(Debug, Clone)]
pub struct MentalCommand {
    /// The detected action name (e.g., "push", "pull", "neutral").
    pub action: String,
    /// Action intensity 0.0–1.0.
    pub power: f32,
}

/// A facial expression event from the "fac" stream.
#[derive(Debug, Deserialize)]
pub struct FacEvent {
    /// Session ID.
    pub sid: String,

    /// Timestamp (Unix seconds as f64, from Cortex).
    pub time: f64,

    /// Facial expression data.
    pub fac: Vec<serde_json::Value>,
}

/// Parsed facial expression data from a "fac" stream event.
#[derive(Debug, Clone)]
pub struct FacialExpression {
    /// Eye action (e.g., "blink", "winkL", "winkR", "lookL", "lookR").
    pub eye_action: String,
    /// Upper face action (e.g., "surprise", "frown").
    pub upper_face_action: String,
    /// Upper face action power 0.0–1.0.
    pub upper_face_power: f32,
    /// Lower face action (e.g., "smile", "clench").
    pub lower_face_action: String,
    /// Lower face action power 0.0–1.0.
    pub lower_face_power: f32,
}

/// A system event from the "sys" stream.
///
/// Used during training for mental commands and facial expressions.
#[derive(Debug, Deserialize)]
pub struct SysEvent {
    /// Session ID.
    pub sid: String,

    /// Timestamp (Unix seconds as f64, from Cortex).
    pub time: f64,

    /// System event data: `[event_type, message]`.
    pub sys: Vec<serde_json::Value>,
}

/// A generic data event from a subscribed stream.
///
/// Used by the reader loop to detect which stream type a message belongs to.
/// Each field is `Some` only when the corresponding stream is active.
#[derive(Debug, Deserialize)]
pub struct StreamEvent {
    /// Session ID.
    pub sid: Option<String>,

    /// Timestamp.
    pub time: Option<f64>,

    /// EEG data (present when subscribed to "eeg").
    pub eeg: Option<Vec<serde_json::Value>>,

    /// Device data / contact quality (present when subscribed to "dev").
    pub dev: Option<Vec<serde_json::Value>>,

    /// Motion data (present when subscribed to "mot").
    pub mot: Option<Vec<f64>>,

    /// EEG quality data (present when subscribed to "eq").
    pub eq: Option<Vec<serde_json::Value>>,

    /// Band power data (present when subscribed to "pow").
    pub pow: Option<Vec<f64>>,

    /// Performance metrics (present when subscribed to "met").
    pub met: Option<Vec<serde_json::Value>>,

    /// Mental command data (present when subscribed to "com").
    pub com: Option<Vec<serde_json::Value>>,

    /// Facial expression data (present when subscribed to "fac").
    pub fac: Option<Vec<serde_json::Value>>,

    /// System events (present when subscribed to "sys").
    pub sys: Option<Vec<serde_json::Value>>,
}

// ─── Records & Markers ──────────────────────────────────────────────────

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
    pub fn as_str(&self) -> &'static str {
        match self {
            ExportFormat::Csv => "CSV",
            ExportFormat::Edf => "EDF",
        }
    }
}

// ─── Profiles ───────────────────────────────────────────────────────────

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

// ─── Training ───────────────────────────────────────────────────────────

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

// ─── Authentication ─────────────────────────────────────────────────────

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

// ─── Subjects ───────────────────────────────────────────────────────────

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

// ─── Advanced BCI Types ─────────────────────────────────────────────────

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

// ─── Method Names ───────────────────────────────────────────────────────

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
        assert_eq!(info.extra.get("futureField"), Some(&serde_json::json!("future")));
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
    fn test_deserialize_eeg_event() {
        // Real Cortex V2 format: MARKERS is an array, not a number
        let json = r#"{
            "sid": "session-uuid-123",
            "time": 1609459200.123456,
            "eeg": [29, 0, 4262.564, 4264.615, 4265.128, 4267.179, 4263.59, 0.0, 0, []]
        }"#;

        let event: EegEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.sid, "session-uuid-123");
        assert_eq!(event.eeg.len(), 10);
        assert_eq!(event.eeg[0].as_u64(), Some(29)); // COUNTER
        assert!(event.eeg[9].is_array()); // MARKERS
    }

    #[test]
    fn test_parse_eeg_data_insight() {
        // [COUNTER, INTERPOLATED, AF3, T7, Pz, T8, AF4, RAW_CQ, MARKER_HARDWARE, MARKERS]
        let eeg: Vec<serde_json::Value> = serde_json::from_str(
            r#"[29, 0, 4262.564, 4264.615, 4265.128, 4267.179, 4263.59, 0.0, 0, []]"#,
        )
        .unwrap();

        let data = EegData::from_eeg_array(&eeg, 5, 1609459200.0).unwrap();
        assert_eq!(data.counter, 29);
        assert!(!data.interpolated);
        assert_eq!(data.channels.len(), 5);
        assert!((data.channels[0] - 4262.564).abs() < 0.01);
        assert!((data.channels[4] - 4263.59).abs() < 0.01);
        assert!((data.raw_cq - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_eeg_data_too_short() {
        let eeg: Vec<serde_json::Value> = serde_json::from_str(r#"[29, 0, 4262.564]"#).unwrap();
        assert!(EegData::from_eeg_array(&eeg, 5, 1.0).is_none());
    }

    #[test]
    fn test_parse_eeg_data_with_markers() {
        let eeg: Vec<serde_json::Value> = serde_json::from_str(
            r#"[30, 0, 4100.0, 4200.0, 4300.0, 4400.0, 4500.0, 1.0, 0, ["marker1"]]"#,
        )
        .unwrap();

        let data = EegData::from_eeg_array(&eeg, 5, 2.0).unwrap();
        assert_eq!(data.counter, 30);
        assert_eq!(data.channels.len(), 5);
        assert!((data.raw_cq - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_serialize_request_no_params() {
        // Empty params should be omitted entirely (matching official Cortex examples)
        let req = CortexRequest::new(1, Methods::QUERY_HEADSETS, serde_json::json!({}));

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"queryHeadsets\""));
        assert!(
            !json.contains("\"params\""),
            "empty params should be omitted: {}",
            json
        );
    }

    #[test]
    fn test_serialize_request_with_params() {
        let req = CortexRequest::new(
            1,
            Methods::AUTHORIZE,
            serde_json::json!({"clientId": "abc", "clientSecret": "xyz"}),
        );

        let json = serde_json::to_string(&req).unwrap();
        assert!(
            json.contains("\"params\""),
            "non-empty params should be present: {}",
            json
        );
        assert!(json.contains("\"clientId\":\"abc\""));
    }

    #[test]
    fn test_deserialize_rpc_error() {
        let json = r#"{
            "id": 1,
            "error": {
                "code": -32002,
                "message": "Access denied"
            }
        }"#;

        let resp: CortexResponse = serde_json::from_str(json).unwrap();
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, ErrorCodes::ACCESS_DENIED);
    }

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

    #[test]
    fn test_parse_device_quality_insight() {
        // Insight has 5 channels: AF3, AF4, T7, T8, Pz
        // Format: [battery, signal, AF3_cq, AF4_cq, T7_cq, T8_cq, Pz_cq, overall, battery_pct]
        let dev: Vec<serde_json::Value> =
            serde_json::from_str(r#"[4, 1, 4, 3, 2, 4, 1, 75, 88]"#).unwrap();

        let quality = DeviceQuality::from_dev_array(&dev, 5).unwrap();
        assert_eq!(quality.battery_level, 4);
        assert_eq!(quality.signal_strength, 1.0);
        assert_eq!(quality.channel_quality.len(), 5);
        assert!((quality.channel_quality[0] - 1.0).abs() < f32::EPSILON); // 4/4 = 1.0
        assert!((quality.channel_quality[1] - 0.75).abs() < f32::EPSILON); // 3/4 = 0.75
        assert!((quality.channel_quality[2] - 0.5).abs() < f32::EPSILON); // 2/4 = 0.5
        assert!((quality.overall_quality - 0.75).abs() < f32::EPSILON); // 75/100
        assert_eq!(quality.battery_percent, 88);
    }

    #[test]
    fn test_parse_device_quality_too_short() {
        let dev: Vec<serde_json::Value> = serde_json::from_str(r#"[4, 1]"#).unwrap();
        assert!(DeviceQuality::from_dev_array(&dev, 5).is_none());
    }

    #[test]
    fn test_parse_motion_data() {
        // [COUNTER, INTERPOLATED, Q0, Q1, Q2, Q3, ACCX, ACCY, ACCZ, MAGX, MAGY, MAGZ]
        let mot = vec![
            123.0, 0.0, 0.707, 0.0, 0.707, 0.0, 0.01, -9.81, 0.02, 30.0, -15.0, 45.0,
        ];
        let motion = MotionData::from_mot_array(&mot, 1609459200.0).unwrap();

        let q = motion.quaternion.unwrap();
        assert!((q[0] - 0.707).abs() < 0.001);
        assert!((motion.accelerometer[1] - -9.81).abs() < 0.01);
        assert!((motion.magnetometer[2] - 45.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_band_power() {
        // 5 channels × 5 bands = 25 values
        let mut pow = vec![0.0; 25];
        pow[0] = 1.5; // ch0 theta
        pow[1] = 2.3; // ch0 alpha
        pow[5] = 0.8; // ch1 theta

        let bp = BandPowerData::from_pow_array(&pow, 5, 1609459200.0).unwrap();
        assert_eq!(bp.channel_powers.len(), 5);
        assert!((bp.channel_powers[0][0] - 1.5).abs() < f32::EPSILON); // ch0 theta
        assert!((bp.channel_powers[0][1] - 2.3).abs() < f32::EPSILON); // ch0 alpha
        assert!((bp.channel_powers[1][0] - 0.8).abs() < f32::EPSILON); // ch1 theta
    }

    #[test]
    fn test_deserialize_dev_event() {
        let json = r#"{
            "sid": "session-uuid-123",
            "time": 1609459200.0,
            "dev": [4, 1, 4, 3, 2, 4, 1, 75, 88]
        }"#;

        let event: DevEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.sid, "session-uuid-123");
        assert_eq!(event.dev.len(), 9);
    }

    #[test]
    fn test_deserialize_mot_event() {
        let json = r#"{
            "sid": "session-uuid-123",
            "time": 1609459200.0,
            "mot": [0.0, 0.0, 0.707, 0.0, 0.707, 0.0, 0.01, -9.81, 0.02, 30.0, -15.0, 45.0]
        }"#;

        let event: MotEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.mot.len(), 12);
    }

    #[test]
    fn test_deserialize_pow_event() {
        let json = r#"{
            "sid": "session-uuid-123",
            "time": 1609459200.0,
            "pow": [1.5, 2.3, 0.8, 1.1, 0.5]
        }"#;

        let event: PowEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.pow.len(), 5);
    }

    #[test]
    fn test_deserialize_stream_event_eeg() {
        let json = r#"{
            "sid": "s1",
            "time": 1.0,
            "eeg": [29, 0, 4262.564, 4264.615, 4265.128, 4267.179, 4263.59, 0.0, 0, []]
        }"#;

        let event: StreamEvent = serde_json::from_str(json).unwrap();
        assert!(event.eeg.is_some());
        assert_eq!(event.eeg.as_ref().unwrap().len(), 10);
        assert!(event.dev.is_none());
        assert!(event.mot.is_none());
    }

    #[test]
    fn test_deserialize_stream_event_dev() {
        let json = r#"{
            "sid": "s1",
            "time": 1.0,
            "dev": [4, 1, 3, 2, 1, 0, 4, 50, 75]
        }"#;

        let event: StreamEvent = serde_json::from_str(json).unwrap();
        assert!(event.eeg.is_none());
        assert!(event.dev.is_some());
    }

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
        assert_eq!(profile.extra.get("extraField"), Some(&serde_json::json!("x")));
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

    #[test]
    fn test_export_format_strings() {
        assert_eq!(ExportFormat::Csv.as_str(), "CSV");
        assert_eq!(ExportFormat::Edf.as_str(), "EDF");
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
    fn test_get_user_info_method_name() {
        assert_eq!(Methods::GET_USER_INFO, "getUserInformation");
    }

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
