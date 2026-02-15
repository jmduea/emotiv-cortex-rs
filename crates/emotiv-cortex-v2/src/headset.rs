//! # Headset Model Identification & Channel Configuration
//!
//! Provides [`HeadsetModel`] for identifying Emotiv headset variants from
//! their Cortex ID string, and [`HeadsetChannelConfig`] for getting the
//! standard EEG channel layout for each model.
//!
//! ## Supported Headsets
//!
//! | Model | Channels | Sample Rate | Electrode Positions |
//! |-------|----------|-------------|---------------------|
//! | Insight | 5 | 128 Hz | AF3, AF4, T7, T8, Pz |
//! | EPOC+ | 14 | 128 Hz | Full 10-20 coverage |
//! | EPOC X | 14 | 256 Hz | Full 10-20 coverage |
//! | EPOC Flex | 14 | 128 Hz | Full 10-20 coverage |
//!
//! ## Usage
//!
//! ```
//! use emotiv_cortex_v2::headset::HeadsetModel;
//!
//! let model = HeadsetModel::from_headset_id("INSIGHT-A1B2C3D4");
//! assert_eq!(model, HeadsetModel::Insight);
//! assert_eq!(model.num_channels(), 5);
//! assert_eq!(model.sampling_rate_hz(), 128.0);
//! ```

use serde::{Deserialize, Serialize};

use crate::protocol::headset::HeadsetInfo;

/// Emotiv headset model identifier.
///
/// Inferred from the headset ID string returned by `queryHeadsets`.
/// Emotiv headset IDs follow patterns like `INSIGHT-XXXXXXXX`,
/// `EPOCX-XXXXXXXX`, `EPOCPLUS-XXXXXXXX`, etc.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeadsetModel {
    /// Emotiv Insight — 5 EEG channels at 128 Hz.
    /// Channels: AF3, AF4, T7, T8, Pz.
    Insight,

    /// Emotiv EPOC+ — 14 EEG channels at 128 Hz.
    /// Full 10-20 coverage.
    EpocPlus,

    /// Emotiv EPOC X — 14 EEG channels at 256 Hz.
    /// Same electrode positions as EPOC+, higher sampling rate.
    EpocX,

    /// Emotiv EPOC Flex — configurable up to 32 channels.
    /// Default configuration uses the same 14-channel EPOC+ layout.
    EpocFlex,

    /// Unknown or unrecognized Emotiv headset.
    Unknown(String),
}

/// EEG channel configuration for a headset model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadsetChannelConfig {
    /// Per-channel information (name, electrode position).
    pub channels: Vec<ChannelInfo>,

    /// Sampling rate in Hz (e.g. 128.0 for Insight, 256.0 for EPOC X).
    pub sampling_rate_hz: f64,

    /// ADC resolution in bits.
    pub resolution_bits: u32,
}

/// Information about a single EEG channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    /// Channel name (e.g. "AF3", "T7", "Pz").
    pub name: String,

    /// Standard 10-20 system electrode position.
    /// Same as `name` for standard placements.
    pub position_10_20: Option<String>,
}

// ─── Insight channel names ──────────────────────────────────────────────

const INSIGHT_CHANNELS: &[&str] = &["AF3", "AF4", "T7", "T8", "Pz"];

// ─── EPOC 14-channel layout (EPOC+, EPOC X, EPOC Flex default) ─────────

const EPOC_CHANNELS: &[&str] = &[
    "AF3", "F7", "F3", "FC5", "T7", "P7", "O1", "O2", "P8", "T8", "FC6", "F4", "F8", "AF4",
];

// ─── HeadsetModel impl ─────────────────────────────────────────────────

impl HeadsetModel {
    /// Infer the headset model from a headset ID string.
    ///
    /// Emotiv headset IDs follow the pattern `MODEL-SERIAL` where MODEL
    /// is one of INSIGHT, EPOCPLUS, EPOCX, EPOCFLEX, EPOC+, EPOC-X, etc.
    ///
    /// ```
    /// use emotiv_cortex_v2::headset::HeadsetModel;
    ///
    /// assert_eq!(HeadsetModel::from_headset_id("INSIGHT-12345678"), HeadsetModel::Insight);
    /// assert_eq!(HeadsetModel::from_headset_id("EPOCX-AABBCCDD"), HeadsetModel::EpocX);
    /// assert_eq!(HeadsetModel::from_headset_id("EPOCPLUS-99887766"), HeadsetModel::EpocPlus);
    /// ```
    #[must_use]
    pub fn from_headset_id(headset_id: &str) -> Self {
        let id_upper = headset_id.to_uppercase();

        if id_upper.starts_with("INSIGHT") {
            HeadsetModel::Insight
        } else if id_upper.starts_with("EPOCX") || id_upper.starts_with("EPOC-X") {
            HeadsetModel::EpocX
        } else if id_upper.starts_with("EPOCFLEX") {
            HeadsetModel::EpocFlex
        } else if id_upper.starts_with("EPOCPLUS")
            || id_upper.starts_with("EPOC+")
            || id_upper.starts_with("EPOC")
        {
            // Generic EPOC — assume EPOC+ layout
            HeadsetModel::EpocPlus
        } else {
            HeadsetModel::Unknown(headset_id.to_string())
        }
    }

    /// Infer the headset model from a [`HeadsetInfo`] response.
    #[must_use]
    pub fn from_headset_info(info: &HeadsetInfo) -> Self {
        Self::from_headset_id(&info.id)
    }

    /// Get the standard EEG channel configuration for this headset model.
    #[must_use]
    pub fn channel_config(&self) -> HeadsetChannelConfig {
        let (names, rate): (&[&str], f64) = match self {
            HeadsetModel::Insight | HeadsetModel::Unknown(_) => (INSIGHT_CHANNELS, 128.0),
            HeadsetModel::EpocPlus | HeadsetModel::EpocFlex => (EPOC_CHANNELS, 128.0),
            HeadsetModel::EpocX => (EPOC_CHANNELS, 256.0),
        };

        HeadsetChannelConfig {
            channels: names
                .iter()
                .map(|&n| ChannelInfo {
                    name: n.to_string(),
                    position_10_20: Some(n.to_string()),
                })
                .collect(),
            sampling_rate_hz: rate,
            resolution_bits: 14,
        }
    }

    /// Number of EEG channels for this headset model.
    #[must_use]
    pub fn num_channels(&self) -> usize {
        match self {
            HeadsetModel::Insight | HeadsetModel::Unknown(_) => INSIGHT_CHANNELS.len(),
            HeadsetModel::EpocPlus | HeadsetModel::EpocX | HeadsetModel::EpocFlex => {
                EPOC_CHANNELS.len()
            }
        }
    }

    /// Sampling rate in Hz for this headset model.
    #[must_use]
    pub fn sampling_rate_hz(&self) -> f64 {
        match self {
            HeadsetModel::EpocX => 256.0,
            _ => 128.0,
        }
    }

    /// Channel names for this headset model.
    #[must_use]
    pub fn channel_names(&self) -> &[&str] {
        match self {
            HeadsetModel::Insight | HeadsetModel::Unknown(_) => INSIGHT_CHANNELS,
            HeadsetModel::EpocPlus | HeadsetModel::EpocX | HeadsetModel::EpocFlex => EPOC_CHANNELS,
        }
    }
}

impl std::fmt::Display for HeadsetModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeadsetModel::Insight => write!(f, "Emotiv Insight"),
            HeadsetModel::EpocPlus => write!(f, "Emotiv EPOC+"),
            HeadsetModel::EpocX => write!(f, "Emotiv EPOC X"),
            HeadsetModel::EpocFlex => write!(f, "Emotiv EPOC Flex"),
            HeadsetModel::Unknown(id) => write!(f, "Unknown Emotiv ({id})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Model inference ────────────────────────────────────────────────

    #[test]
    fn test_infer_insight() {
        assert_eq!(
            HeadsetModel::from_headset_id("INSIGHT-A1B2C3D4"),
            HeadsetModel::Insight
        );
    }

    #[test]
    fn test_infer_insight_lowercase() {
        assert_eq!(
            HeadsetModel::from_headset_id("insight-12345678"),
            HeadsetModel::Insight
        );
    }

    #[test]
    fn test_infer_epocx() {
        assert_eq!(
            HeadsetModel::from_headset_id("EPOCX-12345678"),
            HeadsetModel::EpocX
        );
    }

    #[test]
    fn test_infer_epoc_dash_x() {
        assert_eq!(
            HeadsetModel::from_headset_id("EPOC-X-12345678"),
            HeadsetModel::EpocX
        );
    }

    #[test]
    fn test_infer_epocplus() {
        assert_eq!(
            HeadsetModel::from_headset_id("EPOCPLUS-AABBCCDD"),
            HeadsetModel::EpocPlus
        );
    }

    #[test]
    fn test_infer_epoc_plus_symbol() {
        assert_eq!(
            HeadsetModel::from_headset_id("EPOC+-AABBCCDD"),
            HeadsetModel::EpocPlus
        );
    }

    #[test]
    fn test_infer_epocflex() {
        assert_eq!(
            HeadsetModel::from_headset_id("EPOCFLEX-11223344"),
            HeadsetModel::EpocFlex
        );
    }

    #[test]
    fn test_infer_generic_epoc() {
        assert_eq!(
            HeadsetModel::from_headset_id("EPOC-DEADBEEF"),
            HeadsetModel::EpocPlus
        );
    }

    #[test]
    fn test_infer_unknown() {
        assert_eq!(
            HeadsetModel::from_headset_id("MNEXYZ-12345678"),
            HeadsetModel::Unknown("MNEXYZ-12345678".into())
        );
    }

    #[test]
    fn test_from_headset_info() {
        let info = HeadsetInfo {
            status: "connected".into(),
            id: "INSIGHT-AAAA0000".into(),
            connected_by: None,
            custom_name: None,
            dongle_serial: None,
            firmware: None,
            motion_sensors: None,
            sensors: None,
            settings: None,
            flex_mapping: None,
            headband_position: None,
            is_virtual: None,
            mode: None,
            battery_percent: None,
            signal_strength: None,
            power: None,
            virtual_headset_id: None,
            firmware_display: None,
            is_dfu_mode: None,
            dfu_types: None,
            system_up_time: None,
            uptime: None,
            bluetooth_up_time: None,
            counter: None,
            extra: std::collections::HashMap::new(),
        };
        assert_eq!(
            HeadsetModel::from_headset_info(&info),
            HeadsetModel::Insight
        );
    }

    // ─── Channel config ─────────────────────────────────────────────────

    #[test]
    fn test_insight_channels() {
        let model = HeadsetModel::Insight;
        assert_eq!(model.num_channels(), 5);
        assert_eq!(model.sampling_rate_hz(), 128.0);

        let config = model.channel_config();
        assert_eq!(config.channels.len(), 5);
        assert_eq!(config.sampling_rate_hz, 128.0);
        assert_eq!(config.resolution_bits, 14);
        assert_eq!(config.channels[0].name, "AF3");
        assert_eq!(config.channels[4].name, "Pz");
    }

    #[test]
    fn test_epocplus_channels() {
        let model = HeadsetModel::EpocPlus;
        assert_eq!(model.num_channels(), 14);
        assert_eq!(model.sampling_rate_hz(), 128.0);

        let config = model.channel_config();
        assert_eq!(config.channels.len(), 14);
        assert_eq!(config.sampling_rate_hz, 128.0);
        assert_eq!(config.channels[0].name, "AF3");
        assert_eq!(config.channels[13].name, "AF4");
    }

    #[test]
    fn test_epocx_channels() {
        let model = HeadsetModel::EpocX;
        assert_eq!(model.num_channels(), 14);
        assert_eq!(model.sampling_rate_hz(), 256.0);

        let config = model.channel_config();
        assert_eq!(config.channels.len(), 14);
        assert_eq!(config.sampling_rate_hz, 256.0);
    }

    #[test]
    fn test_epocflex_channels() {
        let model = HeadsetModel::EpocFlex;
        assert_eq!(model.num_channels(), 14);
        assert_eq!(model.sampling_rate_hz(), 128.0);
    }

    #[test]
    fn test_unknown_falls_back_to_insight() {
        let model = HeadsetModel::Unknown("FOO-123".into());
        assert_eq!(model.num_channels(), 5);
        assert_eq!(model.sampling_rate_hz(), 128.0);
    }

    // ─── Channel names ──────────────────────────────────────────────────

    #[test]
    fn test_channel_names() {
        assert_eq!(HeadsetModel::Insight.channel_names(), INSIGHT_CHANNELS);
        assert_eq!(HeadsetModel::EpocPlus.channel_names(), EPOC_CHANNELS);
        assert_eq!(HeadsetModel::EpocX.channel_names(), EPOC_CHANNELS);
    }

    // ─── Display ────────────────────────────────────────────────────────

    #[test]
    fn test_display() {
        assert_eq!(HeadsetModel::Insight.to_string(), "Emotiv Insight");
        assert_eq!(HeadsetModel::EpocPlus.to_string(), "Emotiv EPOC+");
        assert_eq!(HeadsetModel::EpocX.to_string(), "Emotiv EPOC X");
        assert_eq!(HeadsetModel::EpocFlex.to_string(), "Emotiv EPOC Flex");
        assert_eq!(
            HeadsetModel::Unknown("FOO".into()).to_string(),
            "Unknown Emotiv (FOO)"
        );
    }
}
