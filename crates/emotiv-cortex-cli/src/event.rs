//! Application events bridging async data streams and terminal input.
//!
//! [`AppEvent`] is the single event type consumed by the main TUI loop.
//! Terminal events arrive via crossterm's `EventStream`, stream data via
//! `tokio::sync::mpsc` channels fed by async subscriber tasks, and ticks
//! from a periodic timer to drive rendering at a steady frame rate.

use emotiv_cortex_v2::headset::HeadsetModel;
use emotiv_cortex_v2::protocol::headset::HeadsetInfo;
use emotiv_cortex_v2::protocol::streams::{
    BandPowerData, DeviceQuality, EegData, EegQuality, FacialExpression, MentalCommand, MotionData,
    PerformanceMetrics,
};

/// Every event the TUI main loop can receive.
#[derive(Debug)]
#[allow(dead_code)]
pub enum AppEvent {
    /// A crossterm terminal event (key press, mouse, resize).
    Terminal(crossterm::event::Event),
    /// Render tick — triggers a frame redraw.
    Tick,

    // ── Stream data ──────────────────────────────────────────────────
    /// Raw EEG sample.
    Eeg(EegData),
    /// Device contact quality / battery / signal.
    DeviceQuality(DeviceQuality),
    /// IMU / motion data.
    Motion(MotionData),
    /// Per-channel band power.
    BandPower(BandPowerData),
    /// Performance metrics (engagement, stress, focus, …).
    Metrics(PerformanceMetrics),
    /// Mental command action + power.
    MentalCommand(MentalCommand),
    /// Facial expression actions + powers.
    FacialExpression(FacialExpression),
    /// EEG quality (per-sensor signal quality).
    EegQuality(EegQuality),

    // ── Lifecycle ────────────────────────────────────────────────────
    /// A headset query returned new info.
    HeadsetUpdate(Vec<HeadsetInfo>),
    /// Authentication completed — headsets discovered, awaiting user selection.
    AuthReady { token: String },
    /// Headset connected + session created — streams can now be subscribed.
    ConnectionReady {
        token: String,
        session_id: String,
        headset_id: String,
        model: HeadsetModel,
    },
    /// Informational / error log entry.
    Log(LogEntry),
    /// Request application quit.
    Quit,

    // ── LSL ──────────────────────────────────────────────────────────
    /// LSL streaming started successfully.
    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    LslStarted(crate::lsl::LslStreamingHandle),
    /// LSL streaming stopped.
    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    LslStopped,
}

/// Severity levels for log entries shown in the Log tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

/// A single log entry for the scrollable log panel.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: std::time::Instant,
    pub level: LogLevel,
    pub message: String,
}

impl LogEntry {
    pub fn info(msg: impl Into<String>) -> Self {
        Self {
            timestamp: std::time::Instant::now(),
            level: LogLevel::Info,
            message: msg.into(),
        }
    }

    pub fn warn(msg: impl Into<String>) -> Self {
        Self {
            timestamp: std::time::Instant::now(),
            level: LogLevel::Warn,
            message: msg.into(),
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            timestamp: std::time::Instant::now(),
            level: LogLevel::Error,
            message: msg.into(),
        }
    }
}
