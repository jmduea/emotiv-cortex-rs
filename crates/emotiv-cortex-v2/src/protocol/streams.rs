//! Stream event and parsed stream payload protocol types.

use serde::Deserialize;

fn f64_to_f32(value: f64) -> Option<f32> {
    if !value.is_finite() {
        return None;
    }
    value.to_string().parse::<f32>().ok()
}

fn seconds_to_micros_i64(timestamp_secs: f64) -> Option<i64> {
    if !timestamp_secs.is_finite() {
        return None;
    }
    let micros = timestamp_secs * 1_000_000.0;
    if !micros.is_finite() {
        return None;
    }
    format!("{micros:.0}").parse::<i64>().ok()
}

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
    #[must_use]
    pub fn from_eeg_array(
        eeg: &[serde_json::Value],
        num_channels: usize,
        timestamp: f64,
    ) -> Option<Self> {
        // COUNTER + INTERPOLATED + channels + RAW_CQ + MARKER_HARDWARE + MARKERS
        if eeg.len() < 2 + num_channels + 3 {
            return None;
        }

        let counter = u32::try_from(eeg[0].as_u64()?).ok()?;
        let interpolated = eeg[1].as_u64()? != 0;

        let channels: Vec<f32> = eeg[2..2 + num_channels]
            .iter()
            .map(|v| v.as_f64().and_then(f64_to_f32))
            .collect::<Option<Vec<f32>>>()?;

        let raw_cq = f64_to_f32(eeg[2 + num_channels].as_f64()?)?;

        Some(Self {
            timestamp: seconds_to_micros_i64(timestamp)?,
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
    #[must_use]
    pub fn from_dev_array(dev: &[serde_json::Value], num_channels: usize) -> Option<Self> {
        // Minimum: battery + signal + num_channels CQ values + overall + battery_pct
        if dev.len() < 2 + num_channels + 2 {
            return None;
        }

        let battery_level = u8::try_from(dev[0].as_u64()?).ok()?;
        let signal_strength = f64_to_f32(dev[1].as_f64()?)?;

        let channel_quality: Vec<f32> = dev[2..2 + num_channels]
            .iter()
            .filter_map(serde_json::Value::as_f64)
            .map(|cq| f64_to_f32(cq / 4.0)) // Normalize 0–4 to 0.0–1.0
            .collect::<Option<Vec<f32>>>()?;

        if channel_quality.len() != num_channels {
            return None;
        }

        let overall_idx = 2 + num_channels;
        let battery_pct_idx = overall_idx + 1;

        let overall_quality = f64_to_f32(dev.get(overall_idx)?.as_f64()? / 100.0)?;
        let battery_percent = u8::try_from(dev.get(battery_pct_idx)?.as_u64()?).ok()?;

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
    #[must_use]
    pub fn from_mot_array(mot: &[f64], timestamp: f64) -> Option<Self> {
        if mot.len() < 12 {
            return None;
        }

        // Skip COUNTER (0) and INTERPOLATED (1), then Q0-Q3, then ACC, then MAG
        Some(Self {
            timestamp: seconds_to_micros_i64(timestamp)?,
            quaternion: Some([
                f64_to_f32(mot[2])?,
                f64_to_f32(mot[3])?,
                f64_to_f32(mot[4])?,
                f64_to_f32(mot[5])?,
            ]),
            accelerometer: [
                f64_to_f32(mot[6])?,
                f64_to_f32(mot[7])?,
                f64_to_f32(mot[8])?,
            ],
            magnetometer: [
                f64_to_f32(mot[9])?,
                f64_to_f32(mot[10])?,
                f64_to_f32(mot[11])?,
            ],
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
    #[must_use]
    pub fn from_eq_array(eq: &[serde_json::Value], num_channels: usize) -> Option<Self> {
        // Minimum: battery + overall + sr_quality + num_channels sensor values
        if eq.len() < 3 + num_channels {
            return None;
        }

        let battery_percent = u8::try_from(eq[0].as_u64()?).ok()?;
        let overall = f64_to_f32(eq[1].as_f64()? / 100.0)?;
        let sample_rate_quality = f64_to_f32(eq[2].as_f64()?)?;

        let sensor_quality: Vec<f32> = eq[3..3 + num_channels]
            .iter()
            .filter_map(serde_json::Value::as_f64)
            .map(|q| f64_to_f32(q / 4.0)) // Normalize 0–4 to 0.0–1.0
            .collect::<Option<Vec<f32>>>()?;

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

    /// Band power values: [`ch1_theta`, `ch1_alpha`, `ch1_betaL`, `ch1_betaH`, `ch1_gamma`, `ch2_theta`, ...].
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
    #[must_use]
    pub fn from_pow_array(pow: &[f64], num_channels: usize, timestamp: f64) -> Option<Self> {
        if pow.len() < num_channels * 5 {
            return None;
        }

        let channel_powers: Vec<[f32; 5]> = pow
            .chunks_exact(5)
            .take(num_channels)
            .map(|chunk| {
                Some([
                    f64_to_f32(chunk[0])?,
                    f64_to_f32(chunk[1])?,
                    f64_to_f32(chunk[2])?,
                    f64_to_f32(chunk[3])?,
                    f64_to_f32(chunk[4])?,
                ])
            })
            .collect::<Option<Vec<[f32; 5]>>>()?;

        Some(Self {
            timestamp: seconds_to_micros_i64(timestamp)?,
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

#[cfg(test)]
mod tests {
    use super::*;

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
            r"[29, 0, 4262.564, 4264.615, 4265.128, 4267.179, 4263.59, 0.0, 0, []]",
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
        let eeg: Vec<serde_json::Value> = serde_json::from_str(r"[29, 0, 4262.564]").unwrap();
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
    fn test_parse_device_quality_insight() {
        // Insight has 5 channels: AF3, AF4, T7, T8, Pz
        // Format: [battery, signal, AF3_cq, AF4_cq, T7_cq, T8_cq, Pz_cq, overall, battery_pct]
        let dev: Vec<serde_json::Value> =
            serde_json::from_str(r"[4, 1, 4, 3, 2, 4, 1, 75, 88]").unwrap();

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
        let dev: Vec<serde_json::Value> = serde_json::from_str(r"[4, 1]").unwrap();
        assert!(DeviceQuality::from_dev_array(&dev, 5).is_none());
    }

    #[test]
    fn test_parse_eq_quality_insight() {
        // [battery_pct, overall_pct, sample_rate_quality, AF3, AF4, T7, T8, Pz]
        let eq: Vec<serde_json::Value> =
            serde_json::from_str(r"[88, 75, 0.9, 4, 3, 2, 1, 4]").unwrap();

        let parsed = EegQuality::from_eq_array(&eq, 5).unwrap();
        assert_eq!(parsed.battery_percent, 88);
        assert!((parsed.overall - 0.75).abs() < f32::EPSILON);
        assert!((parsed.sample_rate_quality - 0.9).abs() < f32::EPSILON);
        assert_eq!(parsed.sensor_quality.len(), 5);
        assert!((parsed.sensor_quality[0] - 1.0).abs() < f32::EPSILON);
        assert!((parsed.sensor_quality[1] - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_eq_quality_too_short() {
        let eq: Vec<serde_json::Value> = serde_json::from_str(r"[88, 75, 1.0, 4]").unwrap();
        assert!(EegQuality::from_eq_array(&eq, 5).is_none());
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
    fn test_deserialize_met_event() {
        let json = r#"{
            "sid": "session-uuid-123",
            "time": 1609459200.0,
            "met": [0.2, 0.3, 0.4, 0.1]
        }"#;

        let event: MetEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.sid, "session-uuid-123");
        assert_eq!(event.met.len(), 4);
    }

    #[test]
    fn test_deserialize_com_event() {
        let json = r#"{
            "sid": "session-uuid-123",
            "time": 1609459200.0,
            "com": ["push", 0.82]
        }"#;

        let event: ComEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.com.len(), 2);
        assert_eq!(event.com[0].as_str(), Some("push"));
    }

    #[test]
    fn test_deserialize_fac_event() {
        let json = r#"{
            "sid": "session-uuid-123",
            "time": 1609459200.0,
            "fac": ["blink", "surprise", 0.9, "smile", 0.7]
        }"#;

        let event: FacEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.fac.len(), 5);
        assert_eq!(event.fac[0].as_str(), Some("blink"));
    }

    #[test]
    fn test_deserialize_sys_event() {
        let json = r#"{
            "sid": "session-uuid-123",
            "time": 1609459200.0,
            "sys": ["mc_action", "start"]
        }"#;

        let event: SysEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.sys.len(), 2);
        assert_eq!(event.sys[0].as_str(), Some("mc_action"));
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
}
