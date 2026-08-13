//! LSL forwarding for `neuroclient-tui`.
//!
//! This module bridges typed Cortex streams to liblsl outlets and publishes
//! structured metadata so generic LSL consumers can understand stream shape
//! without Cortex-specific parsing rules.
//!
//! Metadata written per outlet:
//! - `desc/channels/channel/label`
//! - `desc/channels/channel/unit`
//! - `desc/channels/channel/type`
//! - `desc/channels/channel/location_label` (EEG 10-20 label where available)
//! - `desc/channels/channel/location/{X,Y,Z}` (EEG coordinates in millimeters)
//! - `desc/acquisition/*` and `desc/source/*` provenance fields
//! - `desc/reference/*` for EEG (`scheme=unknown`)
//! - `desc/cap/labelscheme` for EEG (`10-20`)
//!
//! Stream type mapping:
//! - `EmotivEEG` -> `EEG`
//! - `EmotivMotion` -> `MoCap`
//! - `EmotivBandPower` -> `EEG`
//! - `EmotivMetrics` -> `Metrics`
//! - `EmotivMentalCommands` -> `Markers` (numeric power)
//! - `EmotivMentalCommandMarkers` -> `Markers` (string action labels)
//! - `EmotivFacialExpressions` -> `Markers` (numeric powers)
//! - `EmotivFacialExpressionMarkers` -> `Markers` (string action labels)
//! - `EmotivDeviceQuality` -> `Quality`
//! - `EmotivEEGQuality` -> `Quality`
//!
//! Mental-command and facial-expression events are published as *paired*
//! outlets: an irregular-rate string marker outlet carrying the action
//! labels, and a numeric outlet carrying the power values. Both samples
//! of one Cortex event share a single LSL timestamp, and each outlet
//! declares `desc/pairing/{role,partner_stream}` so consumers can align
//! them without Cortex-specific rules.
//!
//! Sample payload values and channel ordering remain unchanged.

use futures_util::StreamExt;
use neuroclient::CortexClient;
use neuroclient::headset::HeadsetModel;
use neuroclient::protocol::constants::Streams;
use neuroclient::streams;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc as std_mpsc;
use std::thread::JoinHandle as ThreadJoinHandle;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

// ─── Stderr suppression ─────────────────────────────────────────────────
//
// liblsl's C code prints multicast-bind warnings directly to stderr,
// bypassing Rust's tracing/logging.  In a TUI this corrupts the
// alternate screen.  We redirect stderr to NUL around liblsl calls
// that are known to emit these warnings.

/// Temporarily redirects stderr to `NUL` (Windows) or `/dev/null` (Unix).
///
/// Returns a guard that restores stderr on drop.  If any OS call fails
/// we silently do nothing — better to show the warnings than crash.
struct StderrSuppressor {
    #[cfg(windows)]
    saved_fd: Option<i32>,
    #[cfg(not(windows))]
    saved_fd: Option<i32>,
}

impl StderrSuppressor {
    #[allow(unsafe_code)]
    fn new() -> Self {
        #[cfg(windows)]
        {
            // SAFETY: These are standard CRT calls with no invariants
            // beyond valid file descriptors.
            unsafe {
                use std::ffi::CString;
                let saved = libc::dup(2);
                if saved == -1 {
                    return Self { saved_fd: None };
                }
                let nul = CString::new("NUL").expect("static");
                let nul_fd = libc::open(nul.as_ptr(), libc::O_WRONLY);
                if nul_fd == -1 {
                    libc::close(saved);
                    return Self { saved_fd: None };
                }
                libc::dup2(nul_fd, 2);
                libc::close(nul_fd);
                Self {
                    saved_fd: Some(saved),
                }
            }
        }
        #[cfg(not(windows))]
        {
            // SAFETY: Same — standard POSIX fd manipulation.
            unsafe {
                use std::ffi::CString;
                let saved = libc::dup(2);
                if saved == -1 {
                    return Self { saved_fd: None };
                }
                let nul = CString::new("/dev/null").expect("static");
                let nul_fd = libc::open(nul.as_ptr(), libc::O_WRONLY);
                if nul_fd == -1 {
                    libc::close(saved);
                    return Self { saved_fd: None };
                }
                libc::dup2(nul_fd, 2);
                libc::close(nul_fd);
                Self {
                    saved_fd: Some(saved),
                }
            }
        }
    }
}

impl Drop for StderrSuppressor {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        if let Some(saved) = self.saved_fd.take() {
            // SAFETY: Restoring fd 2 from a previously dup'd descriptor.
            unsafe {
                libc::dup2(saved, 2);
                libc::close(saved);
            }
        }
    }
}

/// Prepare liblsl for use.
///
/// Currently a no-op — we rely on liblsl's built-in defaults (ResolveScope =
/// site, standard multicast address pools). This matches LabRecorder's
/// configuration and ensures the hub's inlet can discover our outlet.
///
/// Any warnings liblsl emits about Hyper-V / VPN adapters failing to bind
/// multicast are harmless and suppressed by liblsl's default log level.
fn configure_lsl() {
    // Intentionally empty — use liblsl defaults.
}

/// Channel payload for one LSL sample: numeric or string values.
#[derive(Debug, Clone, PartialEq)]
enum SampleValues {
    /// Numeric sample for `Float32` outlets.
    Floats(Vec<f32>),
    /// String sample for `String` (marker) outlets.
    Strings(Vec<String>),
}

/// One sample plus its LSL capture timestamp.
///
/// A timestamp of `0.0` means "stamp at push time" (liblsl convention).
/// Paired marker/power emissions carry an explicit shared timestamp.
#[derive(Debug, Clone, PartialEq)]
struct TimestampedSample {
    timestamp: f64,
    values: SampleValues,
}

impl TimestampedSample {
    /// Numeric sample stamped at push time.
    fn floats(values: Vec<f32>) -> Self {
        Self {
            timestamp: 0.0,
            values: SampleValues::Floats(values),
        }
    }
}

/// Owns a single LSL outlet on a dedicated OS thread and accepts samples via channel.
struct OutletWorker {
    sample_tx: mpsc::Sender<TimestampedSample>,
    thread_handle: ThreadJoinHandle<()>,
}

/// Which streams to forward to LSL
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LslStream {
    /// Raw EEG voltage samples (channel count/rate based on headset model).
    Eeg,
    /// Motion/IMU samples (accelerometer, magnetometer, quaternion).
    Motion,
    /// Flattened band-power features in channel-major order.
    BandPower,
    /// Performance metrics (engagement, stress, attention, etc.).
    Metrics,
    /// Mental command power (numeric marker-like stream).
    MentalCommands,
    /// Facial expression power features (numeric marker-like stream).
    FacialExpressions,
    /// Contact quality and battery/signal health.
    DeviceQuality,
    /// EEG quality metrics and battery/sample-rate quality.
    EegQuality,
}

impl LslStream {
    /// Human-readable label for menu display.
    ///
    /// For example, `LslStream::BandPower.label()` returns `"Band Power"`.
    pub fn label(&self) -> &'static str {
        match self {
            LslStream::Eeg => "EEG",
            LslStream::Motion => "Motion",
            LslStream::BandPower => "Band Power",
            LslStream::Metrics => "Performance Metrics",
            LslStream::MentalCommands => "Mental Commands",
            LslStream::FacialExpressions => "Facial Expressions",
            LslStream::DeviceQuality => "Device Quality",
            LslStream::EegQuality => "EEG Quality",
        }
    }

    /// All available stream variants in stable menu order.
    ///
    /// This ordering is used by the interactive stream-selection menu.
    pub fn all() -> &'static [LslStream] {
        &[
            LslStream::Eeg,
            LslStream::Motion,
            LslStream::BandPower,
            LslStream::Metrics,
            LslStream::MentalCommands,
            LslStream::FacialExpressions,
            LslStream::DeviceQuality,
            LslStream::EegQuality,
        ]
    }
}

/// Per-channel metadata emitted into `StreamInfo.desc().channels`.
#[derive(Debug, Clone)]
struct ChannelMeta {
    /// Canonical channel label in emitted sample order.
    label: String,
    /// Human-readable measurement unit (e.g. `microvolts`, `%`, `none`).
    unit: &'static str,
    /// Channel semantic type following XDF naming conventions where available.
    kind: &'static str,
    /// Optional EEG 10-20 label for spatial channels.
    location_label: Option<String>,
    /// Optional EEG channel coordinates in millimeters.
    location_xyz_mm: Option<[f64; 3]>,
}

/// Pairing metadata linking a marker outlet to its power outlet (and
/// vice versa) for event streams published as paired outlets.
#[derive(Debug, Clone)]
struct PairingMeta {
    /// This outlet's role: `"marker"` (string labels) or `"power"` (numeric).
    role: &'static str,
    /// Name of the partner outlet carrying the other half of each event.
    partner: &'static str,
}

/// Static outlet schema used to build both `StreamInfo` and status summaries.
#[derive(Debug, Clone)]
struct OutletMeta {
    /// LSL outlet name.
    name: &'static str,
    /// LSL stream type used by discovery filters.
    stream_type: &'static str,
    /// Nominal sampling rate (`0.0` for irregular/event-like streams).
    srate: f64,
    /// LSL channel value format (`Float32` for numeric, `String` for markers).
    format: lsl::ChannelFormat,
    /// Ordered channel metadata matching sample payload shape.
    channels: Vec<ChannelMeta>,
    /// Present when this outlet is one half of a marker/power pair.
    pairing: Option<PairingMeta>,
}

/// Helper for scalar metadata channels that do not have a physical location.
fn simple_channel(label: &str, unit: &'static str, kind: &'static str) -> ChannelMeta {
    ChannelMeta {
        label: label.to_string(),
        unit,
        kind,
        location_label: None,
        location_xyz_mm: None,
    }
}

/// Return canonical 10-20 electrode coordinates in millimeters.
fn eeg_position_10_20_xyz_mm(label: &str) -> Option<[f64; 3]> {
    match label {
        "AF3" => Some([-35.0, 76.0, 52.0]),
        "AF4" => Some([35.0, 76.0, 52.0]),
        "F7" => Some([-68.0, 46.0, 40.0]),
        "F3" => Some([-48.0, 52.0, 54.0]),
        "FC5" => Some([-60.0, 22.0, 52.0]),
        "T7" => Some([-84.0, 0.0, 10.0]),
        "P7" => Some([-68.0, -48.0, 36.0]),
        "O1" => Some([-30.0, -84.0, 28.0]),
        "O2" => Some([30.0, -84.0, 28.0]),
        "P8" => Some([68.0, -48.0, 36.0]),
        "T8" => Some([84.0, 0.0, 10.0]),
        "FC6" => Some([60.0, 22.0, 52.0]),
        "F4" => Some([48.0, 52.0, 54.0]),
        "F8" => Some([68.0, 46.0, 40.0]),
        "Pz" => Some([0.0, -58.0, 64.0]),
        _ => None,
    }
}

/// Build the schema contract for a selected outlet stream.
///
/// The returned metadata is the single source of truth for:
/// - `StreamInfo` core fields (name/type/count/rate)
/// - XML channel metadata (`label`, `unit`, `type`, optional `location_label` and `location`)
/// - startup status summaries
fn outlet_meta(stream: LslStream, model: &HeadsetModel) -> OutletMeta {
    match stream {
        LslStream::Eeg => {
            let config = model.channel_config();
            let channels = config
                .channels
                .into_iter()
                .map(|ch| ChannelMeta {
                    label: ch.name,
                    unit: "microvolts",
                    kind: "EEG",
                    location_xyz_mm: ch
                        .position_10_20
                        .as_deref()
                        .and_then(eeg_position_10_20_xyz_mm),
                    location_label: ch.position_10_20,
                })
                .collect();
            OutletMeta {
                name: "EmotivEEG",
                stream_type: "EEG",
                srate: model.sampling_rate_hz(),
                format: lsl::ChannelFormat::Float32,
                channels,
                pairing: None,
            }
        }
        LslStream::Motion => OutletMeta {
            name: "EmotivMotion",
            stream_type: "MoCap",
            srate: 64.0,
            format: lsl::ChannelFormat::Float32,
            pairing: None,
            channels: vec![
                simple_channel("acc_x", "g", "Misc"),
                simple_channel("acc_y", "g", "Misc"),
                simple_channel("acc_z", "g", "Misc"),
                simple_channel("mag_x", "uT", "Misc"),
                simple_channel("mag_y", "uT", "Misc"),
                simple_channel("mag_z", "uT", "Misc"),
                simple_channel("q0", "none", "OrientationA"),
                simple_channel("q1", "none", "OrientationB"),
                simple_channel("q2", "none", "OrientationC"),
                simple_channel("q3", "none", "OrientationD"),
            ],
        },
        LslStream::BandPower => {
            let mut channels = Vec::with_capacity(model.num_channels() * 5);
            for sensor in model.channel_names() {
                for band in ["theta", "alpha", "betaL", "betaH", "gamma"] {
                    channels.push(simple_channel(
                        &format!("{}_{}", sensor, band),
                        "uV2/Hz",
                        "Misc",
                    ));
                }
            }
            OutletMeta {
                name: "EmotivBandPower",
                stream_type: "EEG",
                srate: 0.0,
                format: lsl::ChannelFormat::Float32,
                channels,
                pairing: None,
            }
        }
        LslStream::Metrics => OutletMeta {
            name: "EmotivMetrics",
            stream_type: "Metrics",
            srate: 0.0,
            format: lsl::ChannelFormat::Float32,
            pairing: None,
            channels: vec![
                simple_channel("engagement", "none", "Misc"),
                simple_channel("excitement", "none", "Misc"),
                simple_channel("long_excitement", "none", "Misc"),
                simple_channel("stress", "none", "Misc"),
                simple_channel("relaxation", "none", "Misc"),
                simple_channel("interest", "none", "Misc"),
                simple_channel("attention", "none", "Misc"),
                simple_channel("focus", "none", "Misc"),
            ],
        },
        LslStream::MentalCommands => OutletMeta {
            name: "EmotivMentalCommands",
            stream_type: "Markers",
            srate: 0.0,
            format: lsl::ChannelFormat::Float32,
            channels: vec![simple_channel("command_power", "none", "Stim")],
            pairing: Some(PairingMeta {
                role: "power",
                partner: "EmotivMentalCommandMarkers",
            }),
        },
        LslStream::FacialExpressions => OutletMeta {
            name: "EmotivFacialExpressions",
            stream_type: "Markers",
            srate: 0.0,
            format: lsl::ChannelFormat::Float32,
            channels: vec![
                simple_channel("upper_face_power", "none", "Stim"),
                simple_channel("lower_face_power", "none", "Stim"),
            ],
            pairing: Some(PairingMeta {
                role: "power",
                partner: "EmotivFacialExpressionMarkers",
            }),
        },
        LslStream::DeviceQuality => {
            let mut channels = Vec::with_capacity(model.num_channels() + 3);
            for sensor in model.channel_names() {
                channels.push(simple_channel(
                    &format!("{}_contact_quality", sensor),
                    "none",
                    "Misc",
                ));
            }
            channels.push(simple_channel("battery_percent", "%", "Misc"));
            channels.push(simple_channel("signal_strength", "none", "Misc"));
            channels.push(simple_channel("overall_quality", "none", "Misc"));
            OutletMeta {
                name: "EmotivDeviceQuality",
                stream_type: "Quality",
                srate: 0.0,
                format: lsl::ChannelFormat::Float32,
                channels,
                pairing: None,
            }
        }
        LslStream::EegQuality => {
            let mut channels = Vec::with_capacity(model.num_channels() + 3);
            // Follow the Cortex API `eq` cols order: batteryPercent, overall,
            // sampleRateQuality, then one entry per sensor (bare name, no suffix).
            channels.push(simple_channel("batteryPercent", "%", "Misc"));
            channels.push(simple_channel("overall", "none", "Misc"));
            channels.push(simple_channel("sampleRateQuality", "none", "Misc"));
            for sensor in model.channel_names() {
                channels.push(simple_channel(sensor, "none", "Misc"));
            }
            OutletMeta {
                name: "EmotivEEGQuality",
                stream_type: "Quality",
                srate: 0.0,
                format: lsl::ChannelFormat::Float32,
                channels,
                pairing: None,
            }
        }
    }
}

/// Build the paired string-marker outlet schema for event streams.
///
/// Returns `Some` only for streams that publish paired marker/power
/// outlets ([`LslStream::MentalCommands`] and
/// [`LslStream::FacialExpressions`]).
fn marker_outlet_meta(stream: LslStream) -> Option<OutletMeta> {
    match stream {
        LslStream::MentalCommands => Some(OutletMeta {
            name: "EmotivMentalCommandMarkers",
            stream_type: "Markers",
            srate: 0.0,
            format: lsl::ChannelFormat::String,
            channels: vec![simple_channel("command_action", "none", "Marker")],
            pairing: Some(PairingMeta {
                role: "marker",
                partner: "EmotivMentalCommands",
            }),
        }),
        LslStream::FacialExpressions => Some(OutletMeta {
            name: "EmotivFacialExpressionMarkers",
            stream_type: "Markers",
            srate: 0.0,
            format: lsl::ChannelFormat::String,
            channels: vec![
                simple_channel("eye_action", "none", "Marker"),
                simple_channel("upper_face_action", "none", "Marker"),
                simple_channel("lower_face_action", "none", "Marker"),
            ],
            pairing: Some(PairingMeta {
                role: "marker",
                partner: "EmotivFacialExpressions",
            }),
        }),
        _ => None,
    }
}

/// Marker and power samples derived from one Cortex event, sharing one
/// LSL timestamp.
#[derive(Debug, Clone, PartialEq)]
struct PairedEventSamples {
    /// Shared LSL capture timestamp for both samples.
    timestamp: f64,
    /// String action labels for the marker outlet.
    marker: Vec<String>,
    /// Numeric power values for the power outlet.
    power: Vec<f32>,
}

/// Convert a mental-command event into paired marker/power samples.
///
/// The action label is preserved verbatim (arbitrary custom actions
/// round-trip without loss) and both samples carry `timestamp`.
fn mental_command_event_samples(
    data: &neuroclient::protocol::streams::MentalCommand,
    timestamp: f64,
) -> PairedEventSamples {
    PairedEventSamples {
        timestamp,
        marker: vec![data.action.clone()],
        power: vec![data.power],
    }
}

/// Convert a facial-expression event into paired marker/power samples.
///
/// Eye/upper/lower action labels are preserved verbatim; the power
/// sample carries upper/lower powers in schema order.
fn facial_expression_event_samples(
    data: &neuroclient::protocol::streams::FacialExpression,
    timestamp: f64,
) -> PairedEventSamples {
    PairedEventSamples {
        timestamp,
        marker: vec![
            data.eye_action.clone(),
            data.upper_face_action.clone(),
            data.lower_face_action.clone(),
        ],
        power: vec![data.upper_face_power, data.lower_face_power],
    }
}

/// Create and annotate a liblsl `StreamInfo` using the outlet schema.
///
/// This writes core stream properties and the extended XML metadata tree.
/// For EEG, reference metadata is explicitly marked as `unknown` because the
/// Cortex EEG payload does not provide enough information to derive reference
/// configuration safely.
fn build_stream_info(
    meta: &OutletMeta,
    source_id: &str,
    model: &HeadsetModel,
) -> Result<lsl::StreamInfo, Box<dyn std::error::Error>> {
    let mut info = lsl::StreamInfo::new(
        meta.name,
        meta.stream_type,
        meta.channels.len() as u32,
        meta.srate,
        meta.format,
        source_id,
    )?;

    let mut desc = info.desc();
    let mut channels = desc.append_child("channels");
    for ch in &meta.channels {
        let mut channel = channels.append_child("channel");
        channel = channel.append_child_value("label", &ch.label);
        channel = channel.append_child_value("unit", ch.unit);
        channel = channel.append_child_value("type", ch.kind);
        if let Some(location_label) = &ch.location_label {
            channel = channel.append_child_value("location_label", location_label);
        }
        if let Some([x, y, z]) = ch.location_xyz_mm {
            let mut location = channel.append_child("location");
            location = location.append_child_value("X", &x.to_string());
            location = location.append_child_value("Y", &y.to_string());
            location = location.append_child_value("Z", &z.to_string());
            let _ = location;
        }
        let _ = channel;
    }

    let mut acquisition = desc.append_child("acquisition");
    acquisition = acquisition.append_child_value("manufacturer", "Emotiv");
    acquisition = acquisition.append_child_value("model", &model.to_string());
    let _ = acquisition;

    let mut source = desc.append_child("source");
    source = source.append_child_value("application", "neuroclient-tui");
    source = source.append_child_value("library", "neuroclient");
    source = source.append_child_value("version", env!("CARGO_PKG_VERSION"));
    let _ = source;

    if let Some(pairing) = &meta.pairing {
        let mut pairing_node = desc.append_child("pairing");
        pairing_node = pairing_node.append_child_value("role", pairing.role);
        pairing_node = pairing_node.append_child_value("partner_stream", pairing.partner);
        pairing_node = pairing_node.append_child_value(
            "notes",
            "marker and power samples of one event share a timestamp",
        );
        let _ = pairing_node;
    }

    if meta.name == "EmotivEEG" {
        let mut cap = desc.append_child("cap");
        cap = cap.append_child_value("labelscheme", "10-20");
        let _ = cap;

        let mut reference = desc.append_child("reference");
        reference = reference.append_child_value("scheme", "unknown");
        reference = reference.append_child_value("notes", "not provided by Cortex eeg payload");
        let _ = reference;
    }

    Ok(info)
}

/// Build the XML metadata string for an outlet schema without creating a real outlet.
///
/// Used to populate the TUI XML viewer after streaming starts. Returns an empty
/// string if the stream info cannot be constructed.
fn build_xml_string(meta: &OutletMeta, source_id: &str, model: &HeadsetModel) -> String {
    match build_stream_info(meta, source_id, model) {
        Ok(info) => info
            .to_xml()
            .unwrap_or_default()
            // liblsl (pugixml) indents with \t; ratatui measures tabs as
            // zero-width so the terminal's 8-column tab stops cause text to
            // shift during differential re-renders.  Normalize here once.
            .replace('\t', "  ")
            .replace('\r', ""),
        Err(_) => String::new(),
    }
}

fn spawn_outlet_worker(
    meta: OutletMeta,
    source_id: String,
    model: HeadsetModel,
) -> Result<OutletWorker, Box<dyn std::error::Error>> {
    let (sample_tx, mut sample_rx) = mpsc::channel::<TimestampedSample>(1024);
    let (ready_tx, ready_rx) = std_mpsc::sync_channel::<Result<(), String>>(1);
    let thread_name = format!("lsl-outlet-{}", meta.name);

    let thread_handle = std::thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            // Suppress stderr while liblsl initialises — it prints
            // multicast-bind warnings that corrupt the TUI.
            let _stderr_guard = StderrSuppressor::new();

            let info = match build_stream_info(&meta, &source_id, &model) {
                Ok(info) => info,
                Err(err) => {
                    let _ = ready_tx.send(Err(err.to_string()));
                    return;
                }
            };

            let outlet = match lsl::StreamOutlet::new(&info, 0, 360) {
                Ok(outlet) => {
                    // Drop the suppressor to restore stderr before sending
                    // the ready signal — any subsequent push_sample warnings
                    // are rare and tolerable.
                    drop(_stderr_guard);
                    outlet
                }
                Err(err) => {
                    let _ = ready_tx.send(Err(format!("{err:?}")));
                    return;
                }
            };
            let _ = ready_tx.send(Ok(()));

            while let Some(sample) = sample_rx.blocking_recv() {
                use lsl::ExPushable;
                // A timestamp of 0.0 means "stamp at push time" (liblsl
                // convention); paired samples carry an explicit shared stamp.
                let result = match &sample.values {
                    SampleValues::Floats(values) => {
                        outlet.push_sample_ex(values, sample.timestamp, true)
                    }
                    SampleValues::Strings(values) => {
                        outlet.push_sample_ex(values, sample.timestamp, true)
                    }
                };
                if let Err(err) = result {
                    tracing::warn!("Failed to push LSL sample: {err:?}");
                }
            }
        })?;

    match ready_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => Ok(OutletWorker {
            sample_tx,
            thread_handle,
        }),
        Ok(Err(err)) => Err(err.into()),
        Err(_) => Err("Timed out waiting for LSL outlet worker startup".into()),
    }
}

/// Format a nominal sample rate for status display.
fn format_srate_hz(srate: f64) -> String {
    if srate.fract() == 0.0 {
        format!("{:.0}Hz", srate)
    } else {
        format!("{:.3}Hz", srate)
    }
}

/// Create a compact human-readable outlet schema summary.
///
/// Example: `EmotivEEG [type=EEG, ch=14, srate=128Hz]`
fn format_outlet_summary(meta: &OutletMeta) -> String {
    format!(
        "{} [type={}, ch={}, srate={}]",
        meta.name,
        meta.stream_type,
        meta.channels.len(),
        format_srate_hz(meta.srate),
    )
}

fn register_outlet(
    active_outlets: &mut Vec<String>,
    outlet_workers: &mut Vec<OutletWorker>,
    meta: OutletMeta,
    source_id: &str,
    model: &HeadsetModel,
) -> Result<mpsc::Sender<TimestampedSample>, Box<dyn std::error::Error>> {
    active_outlets.push(format_outlet_summary(&meta));
    let worker = spawn_outlet_worker(meta, source_id.to_string(), model.clone())?;
    let sample_tx = worker.sample_tx.clone();
    outlet_workers.push(worker);
    Ok(sample_tx)
}

/// Handle to a running background LSL streaming session.
///
/// Returned by [`start_lsl_streaming`] and consumed by [`stop_lsl_streaming`].
pub struct LslStreamingHandle {
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
    tasks: Vec<JoinHandle<()>>,
    outlet_workers: Vec<OutletWorker>,
    /// Per-stream (label, counter) pairs for status display.
    pub sample_counts: Arc<Vec<(String, Arc<AtomicU64>)>>,
    /// When streaming was started.
    pub started_at: Instant,
    /// LSL outlet summaries shown in CLI status (name + schema details).
    pub active_streams: Vec<String>,
    /// Which Cortex stream types are subscribed, for unsubscribe on stop.
    subscribed: Vec<LslStream>,
    /// XML metadata strings for each active outlet: `(stream_label, xml_string)`.
    ///
    /// Populated at streaming start from [`build_xml_string`] and displayed by
    /// the TUI XML viewer panel.
    pub stream_xml_metadata: Vec<(String, String)>,
}

impl std::fmt::Debug for LslStreamingHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LslStreamingHandle")
            .field("active_streams", &self.active_streams)
            .field("subscribed", &self.subscribed)
            .finish_non_exhaustive()
    }
}

impl LslStreamingHandle {
    /// Format a compact status string for display in the status bar.
    ///
    /// Example: `LSL ▶ EEG, Motion`
    pub fn format_status(&self) -> String {
        let streams: Vec<&str> = self.subscribed.iter().map(|s| s.label()).collect();
        format!("LSL ▶ {}", streams.join(", "))
    }
}

/// Start LSL streaming in the background.
///
/// Subscribes to selected Cortex streams, creates schema-annotated LSL outlets,
/// and spawns async forwarding tasks. Returns a [`LslStreamingHandle`] that can
/// be used to monitor status and stop streaming later via
/// [`stop_lsl_streaming`].
pub async fn start_lsl_streaming(
    client: &CortexClient,
    token: &str,
    session_id: &str,
    model: &HeadsetModel,
    selected: &[LslStream],
    source_id: &str,
) -> Result<LslStreamingHandle, Box<dyn std::error::Error>> {
    if selected.is_empty() {
        return Err("No streams selected".into());
    }

    configure_lsl();

    let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
    let mut tasks = Vec::new();
    let mut active_outlets = Vec::new();
    let mut outlet_workers = Vec::new();

    // Sample counters for status reporting
    let sample_counts: Arc<Vec<(String, Arc<AtomicU64>)>> = Arc::new(
        selected
            .iter()
            .map(|s| (s.label().to_string(), Arc::new(AtomicU64::new(0))))
            .collect(),
    );

    // XML metadata strings for each selected stream (for TUI display),
    // including the paired string-marker outlets of event streams.
    let stream_xml_metadata: Vec<(String, String)> = selected
        .iter()
        .flat_map(|s| {
            let meta = outlet_meta(*s, model);
            let mut entries = vec![(
                s.label().to_string(),
                build_xml_string(&meta, source_id, model),
            )];
            if let Some(marker_meta) = marker_outlet_meta(*s) {
                entries.push((
                    format!("{} Markers", s.label()),
                    build_xml_string(&marker_meta, source_id, model),
                ));
            }
            entries
        })
        .collect();

    for (idx, stream_type) in selected.iter().enumerate() {
        let mut shutdown_rx = shutdown_tx.subscribe();
        let counter = sample_counts[idx].1.clone();

        match stream_type {
            LslStream::Eeg => {
                let mut stream =
                    streams::subscribe_eeg(client, token, session_id, model.num_channels()).await?;
                let sample_tx = register_outlet(
                    &mut active_outlets,
                    &mut outlet_workers,
                    outlet_meta(LslStream::Eeg, model),
                    source_id,
                    model,
                )?;

                tasks.push(tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            item = stream.next() => {
                                let Some(data) = item else { break };
                                if sample_tx
                                    .send(TimestampedSample::floats(data.channels))
                                    .await
                                    .is_err()
                                {
                                    tracing::warn!("EEG outlet worker stopped");
                                    break;
                                }
                                counter.fetch_add(1, Ordering::Relaxed);
                            }
                            _ = shutdown_rx.recv() => break,
                        }
                    }
                }));
            }

            LslStream::Motion => {
                let mut stream = streams::subscribe_motion(client, token, session_id).await?;
                let sample_tx = register_outlet(
                    &mut active_outlets,
                    &mut outlet_workers,
                    outlet_meta(LslStream::Motion, model),
                    source_id,
                    model,
                )?;

                tasks.push(tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            item = stream.next() => {
                                let Some(data) = item else { break };
                                let mut sample = Vec::with_capacity(10);
                                sample.extend_from_slice(&data.accelerometer);
                                sample.extend_from_slice(&data.magnetometer);
                                if let Some(quat) = data.quaternion {
                                    sample.extend_from_slice(&quat);
                                } else {
                                    sample.extend_from_slice(&[0.0, 0.0, 0.0, 1.0]);
                                }
                                if sample_tx
                                    .send(TimestampedSample::floats(sample))
                                    .await
                                    .is_err()
                                {
                                    tracing::warn!("Motion outlet worker stopped");
                                    break;
                                }
                                counter.fetch_add(1, Ordering::Relaxed);
                            }
                            _ = shutdown_rx.recv() => break,
                        }
                    }
                }));
            }

            LslStream::BandPower => {
                let mut stream =
                    streams::subscribe_band_power(client, token, session_id, model.num_channels())
                        .await?;
                let sample_tx = register_outlet(
                    &mut active_outlets,
                    &mut outlet_workers,
                    outlet_meta(LslStream::BandPower, model),
                    source_id,
                    model,
                )?;

                tasks.push(tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            item = stream.next() => {
                                let Some(data) = item else { break };
                                let sample: Vec<f32> = data.channel_powers.iter().flatten().copied().collect();
                                if sample_tx
                                    .send(TimestampedSample::floats(sample))
                                    .await
                                    .is_err()
                                {
                                    tracing::warn!("Band Power outlet worker stopped");
                                    break;
                                }
                                counter.fetch_add(1, Ordering::Relaxed);
                            }
                            _ = shutdown_rx.recv() => break,
                        }
                    }
                }));
            }

            LslStream::Metrics => {
                let mut stream = streams::subscribe_metrics(client, token, session_id).await?;
                let sample_tx = register_outlet(
                    &mut active_outlets,
                    &mut outlet_workers,
                    outlet_meta(LslStream::Metrics, model),
                    source_id,
                    model,
                )?;

                tasks.push(tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            item = stream.next() => {
                                let Some(data) = item else { break };
                                let sample = vec![
                                    data.engagement.unwrap_or(0.0),
                                    data.excitement.unwrap_or(0.0),
                                    data.long_excitement.unwrap_or(0.0),
                                    data.stress.unwrap_or(0.0),
                                    data.relaxation.unwrap_or(0.0),
                                    data.interest.unwrap_or(0.0),
                                    data.attention.unwrap_or(0.0),
                                    data.focus.unwrap_or(0.0),
                                ];
                                if sample_tx
                                    .send(TimestampedSample::floats(sample))
                                    .await
                                    .is_err()
                                {
                                    tracing::warn!("Metrics outlet worker stopped");
                                    break;
                                }
                                counter.fetch_add(1, Ordering::Relaxed);
                            }
                            _ = shutdown_rx.recv() => break,
                        }
                    }
                }));
            }

            LslStream::MentalCommands => {
                let mut stream =
                    streams::subscribe_mental_commands(client, token, session_id).await?;
                let power_tx = register_outlet(
                    &mut active_outlets,
                    &mut outlet_workers,
                    outlet_meta(LslStream::MentalCommands, model),
                    source_id,
                    model,
                )?;
                let marker_tx = register_outlet(
                    &mut active_outlets,
                    &mut outlet_workers,
                    marker_outlet_meta(LslStream::MentalCommands)
                        .expect("mental commands have a marker outlet"),
                    source_id,
                    model,
                )?;

                tasks.push(tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            item = stream.next() => {
                                let Some(data) = item else { break };
                                // One LSL clock read per event: the marker and
                                // power samples share this timestamp.
                                let paired = mental_command_event_samples(
                                    &data,
                                    lsl::local_clock(),
                                );
                                let marker_sent = marker_tx
                                    .send(TimestampedSample {
                                        timestamp: paired.timestamp,
                                        values: SampleValues::Strings(paired.marker),
                                    })
                                    .await;
                                let power_sent = power_tx
                                    .send(TimestampedSample {
                                        timestamp: paired.timestamp,
                                        values: SampleValues::Floats(paired.power),
                                    })
                                    .await;
                                if marker_sent.is_err() || power_sent.is_err() {
                                    tracing::warn!("Mental Command outlet worker stopped");
                                    break;
                                }
                                counter.fetch_add(1, Ordering::Relaxed);
                            }
                            _ = shutdown_rx.recv() => break,
                        }
                    }
                }));
            }

            LslStream::FacialExpressions => {
                let mut stream =
                    streams::subscribe_facial_expressions(client, token, session_id).await?;
                let power_tx = register_outlet(
                    &mut active_outlets,
                    &mut outlet_workers,
                    outlet_meta(LslStream::FacialExpressions, model),
                    source_id,
                    model,
                )?;
                let marker_tx = register_outlet(
                    &mut active_outlets,
                    &mut outlet_workers,
                    marker_outlet_meta(LslStream::FacialExpressions)
                        .expect("facial expressions have a marker outlet"),
                    source_id,
                    model,
                )?;

                tasks.push(tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            item = stream.next() => {
                                let Some(data) = item else { break };
                                // One LSL clock read per event: the marker and
                                // power samples share this timestamp.
                                let paired = facial_expression_event_samples(
                                    &data,
                                    lsl::local_clock(),
                                );
                                let marker_sent = marker_tx
                                    .send(TimestampedSample {
                                        timestamp: paired.timestamp,
                                        values: SampleValues::Strings(paired.marker),
                                    })
                                    .await;
                                let power_sent = power_tx
                                    .send(TimestampedSample {
                                        timestamp: paired.timestamp,
                                        values: SampleValues::Floats(paired.power),
                                    })
                                    .await;
                                if marker_sent.is_err() || power_sent.is_err() {
                                    tracing::warn!("Facial Expression outlet worker stopped");
                                    break;
                                }
                                counter.fetch_add(1, Ordering::Relaxed);
                            }
                            _ = shutdown_rx.recv() => break,
                        }
                    }
                }));
            }

            LslStream::DeviceQuality => {
                let mut stream =
                    streams::subscribe_dev(client, token, session_id, model.num_channels()).await?;
                let sample_tx = register_outlet(
                    &mut active_outlets,
                    &mut outlet_workers,
                    outlet_meta(LslStream::DeviceQuality, model),
                    source_id,
                    model,
                )?;

                tasks.push(tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            item = stream.next() => {
                                let Some(data) = item else { break };
                                let mut sample = Vec::with_capacity(data.channel_quality.len() + 3);
                                sample.extend_from_slice(&data.channel_quality);
                                sample.push(data.battery_percent as f32);
                                sample.push(data.signal_strength);
                                sample.push(data.overall_quality);
                                if sample_tx
                                    .send(TimestampedSample::floats(sample))
                                    .await
                                    .is_err()
                                {
                                    tracing::warn!("Device Quality outlet worker stopped");
                                    break;
                                }
                                counter.fetch_add(1, Ordering::Relaxed);
                            }
                            _ = shutdown_rx.recv() => break,
                        }
                    }
                }));
            }

            LslStream::EegQuality => {
                let mut stream =
                    streams::subscribe_eq(client, token, session_id, model.num_channels()).await?;
                let sample_tx = register_outlet(
                    &mut active_outlets,
                    &mut outlet_workers,
                    outlet_meta(LslStream::EegQuality, model),
                    source_id,
                    model,
                )?;

                tasks.push(tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            item = stream.next() => {
                                let Some(data) = item else { break };
                                let mut sample = Vec::with_capacity(data.sensor_quality.len() + 3);
                                // Push in API cols order: batteryPercent, overall,
                                // sampleRateQuality, then per-sensor values.
                                sample.push(data.battery_percent as f32);
                                sample.push(data.overall);
                                sample.push(data.sample_rate_quality);
                                sample.extend_from_slice(&data.sensor_quality);
                                if sample_tx
                                    .send(TimestampedSample::floats(sample))
                                    .await
                                    .is_err()
                                {
                                    tracing::warn!("EEG Quality outlet worker stopped");
                                    break;
                                }
                                counter.fetch_add(1, Ordering::Relaxed);
                            }
                            _ = shutdown_rx.recv() => break,
                        }
                    }
                }));
            }
        }
    }

    tracing::info!("LSL streaming active: {}", active_outlets.join(", "));

    Ok(LslStreamingHandle {
        shutdown_tx,
        tasks,
        outlet_workers,
        sample_counts,
        started_at: Instant::now(),
        active_streams: active_outlets,
        subscribed: selected.to_vec(),
        stream_xml_metadata,
    })
}

/// Stop a running LSL streaming session.
///
/// Signals all forwarding tasks to shut down, waits for cleanup, and
/// unsubscribes from the Cortex streams.
pub async fn stop_lsl_streaming(
    handle: LslStreamingHandle,
    client: &CortexClient,
    token: &str,
    session_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Shutting down LSL streaming...");
    let LslStreamingHandle {
        shutdown_tx,
        tasks,
        outlet_workers,
        sample_counts: _,
        started_at: _,
        active_streams: _,
        subscribed,
        stream_xml_metadata: _,
    } = handle;

    // Signal all tasks to stop
    let _ = shutdown_tx.send(());

    // Wait for all tasks to complete with timeout
    let shutdown_timeout = tokio::time::timeout(Duration::from_secs(5), async {
        for task in tasks {
            let _ = task.await;
        }
    })
    .await;

    if shutdown_timeout.is_err() {
        tracing::warn!("Some tasks did not shut down cleanly");
    }

    // Drop worker senders and join outlet threads.
    for worker in outlet_workers {
        drop(worker.sample_tx);
        if worker.thread_handle.join().is_err() {
            tracing::warn!("An LSL outlet thread panicked during shutdown");
        }
    }

    // Unsubscribe from all streams
    let stream_names: Vec<&str> = subscribed
        .iter()
        .map(|s| match s {
            LslStream::Eeg => Streams::EEG,
            LslStream::Motion => Streams::MOT,
            LslStream::BandPower => Streams::POW,
            LslStream::Metrics => Streams::MET,
            LslStream::MentalCommands => Streams::COM,
            LslStream::FacialExpressions => Streams::FAC,
            LslStream::DeviceQuality => Streams::DEV,
            LslStream::EegQuality => Streams::EQ,
        })
        .collect();

    if let Err(e) = streams::unsubscribe(client, token, session_id, &stream_names).await {
        tracing::warn!(
            error_category = e.category(),
            api_code = ?e.api_code(),
            "Failed to unsubscribe"
        );
    }

    tracing::info!("LSL streaming stopped.");
    Ok(())
}

#[cfg(all(test, feature = "lsl"))]
mod tests {
    use super::*;

    fn count_occurrences(haystack: &str, needle: &str) -> usize {
        haystack.match_indices(needle).count()
    }

    #[test]
    fn eeg_streaminfo_contains_sampling_rate_and_channel_locations() {
        let model = HeadsetModel::Insight;
        let meta = outlet_meta(LslStream::Eeg, &model);
        let info = build_stream_info(&meta, "INSIGHT-TEST", &model).unwrap();
        let xml = info.to_xml().unwrap();

        assert_eq!(info.nominal_srate(), model.sampling_rate_hz());
        assert_eq!(info.channel_count() as usize, meta.channels.len());
        assert!(xml.contains("<label>AF3</label>"));
        assert!(xml.contains("<location_label>AF3</location_label>"));
        assert_eq!(count_occurrences(&xml, "<location>"), meta.channels.len());
        assert!(
            meta.channels
                .first()
                .is_some_and(|channel| channel.location_xyz_mm.is_some())
        );
        assert!(xml.contains("<unit>microvolts</unit>"));
        assert!(xml.contains("<type>EEG</type>"));
    }

    #[test]
    fn eeg_streaminfo_declares_reference_unknown() {
        let model = HeadsetModel::Insight;
        let meta = outlet_meta(LslStream::Eeg, &model);
        let info = build_stream_info(&meta, "INSIGHT-TEST", &model).unwrap();
        let xml = info.to_xml().unwrap();

        assert!(xml.contains("<labelscheme>10-20</labelscheme>"));
        assert!(xml.contains("<scheme>unknown</scheme>"));
        assert!(xml.contains("<notes>not provided by Cortex eeg payload</notes>"));
    }

    #[test]
    fn all_streams_have_channel_label_unit_type_entries() {
        let model = HeadsetModel::EpocPlus;
        for &stream in LslStream::all() {
            let meta = outlet_meta(stream, &model);
            let info = build_stream_info(&meta, "STREAM-TEST", &model).unwrap();
            let xml = info.to_xml().unwrap();

            assert_eq!(info.channel_count() as usize, meta.channels.len());
            assert_eq!(count_occurrences(&xml, "<channel>"), meta.channels.len());
            assert_eq!(count_occurrences(&xml, "<label>"), meta.channels.len());
            assert_eq!(count_occurrences(&xml, "<unit>"), meta.channels.len());

            for ch in &meta.channels {
                assert!(xml.contains(&format!("<label>{}</label>", ch.label)));
                assert!(xml.contains(&format!("<unit>{}</unit>", ch.unit)));
                assert!(xml.contains(&format!("<type>{}</type>", ch.kind)));
            }
        }
    }

    #[test]
    fn band_power_labels_match_flatten_order() {
        let model = HeadsetModel::Insight;
        let meta = outlet_meta(LslStream::BandPower, &model);
        let labels: Vec<String> = meta.channels.iter().map(|c| c.label.clone()).collect();

        let mut expected = Vec::new();
        for sensor in model.channel_names() {
            for band in ["theta", "alpha", "betaL", "betaH", "gamma"] {
                expected.push(format!("{}_{}", sensor, band));
            }
        }

        assert_eq!(labels, expected);
    }

    #[test]
    fn metrics_stream_type_is_metrics() {
        let model = HeadsetModel::Insight;
        let meta = outlet_meta(LslStream::Metrics, &model);
        let info = build_stream_info(&meta, "MET-TEST", &model).unwrap();

        assert_eq!(meta.stream_type, "Metrics");
        assert_eq!(info.stream_type(), "Metrics");
    }

    #[test]
    fn quality_streams_use_quality_type() {
        let model = HeadsetModel::Insight;
        for stream in [LslStream::DeviceQuality, LslStream::EegQuality] {
            let meta = outlet_meta(stream, &model);
            let info = build_stream_info(&meta, "QUALITY-TEST", &model).unwrap();

            assert_eq!(meta.stream_type, "Quality");
            assert_eq!(info.stream_type(), "Quality");
        }
    }

    #[test]
    fn motion_stream_uses_mocap_orientation_channel_types() {
        let model = HeadsetModel::Insight;
        let meta = outlet_meta(LslStream::Motion, &model);

        let kinds: Vec<&str> = meta.channels.iter().map(|ch| ch.kind).collect();
        assert_eq!(
            kinds,
            vec![
                "Misc",
                "Misc",
                "Misc",
                "Misc",
                "Misc",
                "Misc",
                "OrientationA",
                "OrientationB",
                "OrientationC",
                "OrientationD"
            ]
        );
    }

    #[test]
    fn markers_streams_use_stim_channel_type() {
        let model = HeadsetModel::Insight;
        for stream in [LslStream::MentalCommands, LslStream::FacialExpressions] {
            let meta = outlet_meta(stream, &model);
            let info = build_stream_info(&meta, "MARKER-TEST", &model).unwrap();
            let xml = info.to_xml().unwrap();

            assert!(meta.channels.iter().all(|c| c.kind == "Stim"));
            assert_eq!(
                count_occurrences(&xml, "<type>Stim</type>"),
                meta.channels.len()
            );
        }
    }

    #[test]
    fn mental_command_samples_round_trip_arbitrary_labels() {
        use neuroclient::protocol::streams::MentalCommand;

        for (action, power) in [
            ("push", 0.42_f32),
            ("neutral", 0.0),
            ("my custom action 🚀", 1.0),
            ("träumen", 0.734_231),
        ] {
            let data = MentalCommand {
                action: action.to_string(),
                power,
            };
            let paired = mental_command_event_samples(&data, 123.456);

            // Labels and powers round-trip without loss or remapping.
            assert_eq!(paired.marker, vec![action.to_string()]);
            assert_eq!(paired.power, vec![power]);
            // Marker and power samples share one timestamp.
            assert!((paired.timestamp - 123.456).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn facial_expression_samples_preserve_eye_upper_lower_identity() {
        use neuroclient::protocol::streams::FacialExpression;

        let data = FacialExpression {
            eye_action: "winkL".to_string(),
            upper_face_action: "surprise".to_string(),
            upper_face_power: 0.61,
            lower_face_action: "clench".to_string(),
            lower_face_power: 0.87,
        };
        let paired = facial_expression_event_samples(&data, 42.0);

        assert_eq!(
            paired.marker,
            vec![
                "winkL".to_string(),
                "surprise".to_string(),
                "clench".to_string()
            ]
        );
        assert_eq!(paired.power, vec![0.61, 0.87]);
        assert!((paired.timestamp - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn marker_outlets_use_string_format_and_declare_pairing() {
        let model = HeadsetModel::Insight;
        for (stream, expected_partner) in [
            (LslStream::MentalCommands, "EmotivMentalCommands"),
            (LslStream::FacialExpressions, "EmotivFacialExpressions"),
        ] {
            let marker_meta = marker_outlet_meta(stream).expect("event stream has marker outlet");
            assert_eq!(marker_meta.format, lsl::ChannelFormat::String);
            assert_eq!(marker_meta.srate, 0.0, "marker outlets are irregular-rate");

            let info = build_stream_info(&marker_meta, "MARKER-TEST", &model).unwrap();
            let xml = info.to_xml().unwrap();
            assert!(xml.contains("<role>marker</role>"), "missing role: {xml}");
            assert!(
                xml.contains(&format!(
                    "<partner_stream>{expected_partner}</partner_stream>"
                )),
                "missing partner: {xml}"
            );
        }

        // Non-event streams have no marker outlet.
        assert!(marker_outlet_meta(LslStream::Eeg).is_none());
        assert!(marker_outlet_meta(LslStream::Motion).is_none());
    }

    #[test]
    fn power_outlets_declare_pairing_back_to_marker_outlets() {
        let model = HeadsetModel::Insight;
        for (stream, expected_partner) in [
            (LslStream::MentalCommands, "EmotivMentalCommandMarkers"),
            (
                LslStream::FacialExpressions,
                "EmotivFacialExpressionMarkers",
            ),
        ] {
            let meta = outlet_meta(stream, &model);
            let info = build_stream_info(&meta, "POWER-TEST", &model).unwrap();
            let xml = info.to_xml().unwrap();
            assert!(xml.contains("<role>power</role>"), "missing role: {xml}");
            assert!(
                xml.contains(&format!(
                    "<partner_stream>{expected_partner}</partner_stream>"
                )),
                "missing partner: {xml}"
            );
        }
    }

    #[test]
    fn facial_power_outlet_has_no_reserved_channel() {
        let model = HeadsetModel::Insight;
        let meta = outlet_meta(LslStream::FacialExpressions, &model);

        let labels: Vec<&str> = meta.channels.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, vec!["upper_face_power", "lower_face_power"]);
    }

    #[test]
    fn startup_schema_summary_format_includes_type_count_rate() {
        let model = HeadsetModel::EpocX;
        let eeg_meta = outlet_meta(LslStream::Eeg, &model);
        let mot_meta = outlet_meta(LslStream::Motion, &model);

        assert_eq!(
            format_outlet_summary(&eeg_meta),
            "EmotivEEG [type=EEG, ch=14, srate=256Hz]"
        );
        assert_eq!(
            format_outlet_summary(&mot_meta),
            "EmotivMotion [type=MoCap, ch=10, srate=64Hz]"
        );
    }
}
