//! LSL forwarding for `emotiv-cortex-cli`.
//!
//! This module bridges typed Cortex streams to liblsl outlets and publishes
//! structured metadata so generic LSL consumers can understand stream shape
//! without Cortex-specific parsing rules.
//!
//! Metadata written per outlet:
//! - `desc/channels/channel/label`
//! - `desc/channels/channel/unit`
//! - `desc/channels/channel/type`
//! - `desc/channels/channel/location` (EEG channels where available)
//! - `desc/acquisition/*` and `desc/source/*` provenance fields
//! - `desc/reference/*` for EEG (`scheme=unknown`)
//!
//! Stream type mapping:
//! - `EmotivEEG` -> `EEG`
//! - `EmotivMotion` -> `MoCap`
//! - `EmotivBandPower` -> `EEG`
//! - `EmotivMetrics` -> `""` (empty/non-standard content type)
//! - `EmotivMentalCommands` -> `Markers`
//! - `EmotivFacialExpressions` -> `Markers`
//! - `EmotivDeviceQuality` -> `EEG`
//! - `EmotivEEGQuality` -> `EEG`
//!
//! Sample payload values and channel ordering remain unchanged.

use colored::Colorize;
use emotiv_cortex_v2::CortexClient;
use emotiv_cortex_v2::headset::HeadsetModel;
use emotiv_cortex_v2::protocol::constants::Streams;
use emotiv_cortex_v2::streams;
use futures_util::StreamExt;
use lsl::Pushable;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc as std_mpsc;
use std::thread::JoinHandle as ThreadJoinHandle;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

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

/// Owns a single LSL outlet on a dedicated OS thread and accepts samples via channel.
struct OutletWorker {
    sample_tx: mpsc::Sender<Vec<f32>>,
    thread_handle: ThreadJoinHandle<()>,
}

/// Which streams to forward to LSL
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Channel semantic type (`eeg`, `misc`, `stim`).
    kind: &'static str,
    /// Optional sensor location for spatial channels (10-20 label).
    location: Option<String>,
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
    /// Ordered channel metadata matching sample payload shape.
    channels: Vec<ChannelMeta>,
}

/// Helper for scalar metadata channels that do not have a physical location.
fn simple_channel(label: &str, unit: &'static str, kind: &'static str) -> ChannelMeta {
    ChannelMeta {
        label: label.to_string(),
        unit,
        kind,
        location: None,
    }
}

/// Build the schema contract for a selected outlet stream.
///
/// The returned metadata is the single source of truth for:
/// - `StreamInfo` core fields (name/type/count/rate)
/// - XML channel metadata (`label`, `unit`, `type`, optional `location`)
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
                    kind: "eeg",
                    location: ch.position_10_20,
                })
                .collect();
            OutletMeta {
                name: "EmotivEEG",
                stream_type: "EEG",
                srate: model.sampling_rate_hz(),
                channels,
            }
        }
        LslStream::Motion => OutletMeta {
            name: "EmotivMotion",
            stream_type: "MoCap",
            srate: 0.0,
            channels: vec![
                simple_channel("acc_x", "g", "misc"),
                simple_channel("acc_y", "g", "misc"),
                simple_channel("acc_z", "g", "misc"),
                simple_channel("mag_x", "uT", "misc"),
                simple_channel("mag_y", "uT", "misc"),
                simple_channel("mag_z", "uT", "misc"),
                simple_channel("q0", "none", "misc"),
                simple_channel("q1", "none", "misc"),
                simple_channel("q2", "none", "misc"),
                simple_channel("q3", "none", "misc"),
            ],
        },
        LslStream::BandPower => {
            let mut channels = Vec::with_capacity(model.num_channels() * 5);
            for sensor in model.channel_names() {
                for band in ["theta", "alpha", "betaL", "betaH", "gamma"] {
                    channels.push(simple_channel(
                        &format!("{}_{}", sensor, band),
                        "uV2/Hz",
                        "misc",
                    ));
                }
            }
            OutletMeta {
                name: "EmotivBandPower",
                stream_type: "EEG",
                srate: 0.0,
                channels,
            }
        }
        LslStream::Metrics => OutletMeta {
            name: "EmotivMetrics",
            stream_type: "",
            srate: 0.0,
            channels: vec![
                simple_channel("engagement", "none", "misc"),
                simple_channel("excitement", "none", "misc"),
                simple_channel("long_excitement", "none", "misc"),
                simple_channel("stress", "none", "misc"),
                simple_channel("relaxation", "none", "misc"),
                simple_channel("interest", "none", "misc"),
                simple_channel("attention", "none", "misc"),
                simple_channel("focus", "none", "misc"),
            ],
        },
        LslStream::MentalCommands => OutletMeta {
            name: "EmotivMentalCommands",
            stream_type: "Markers",
            srate: 0.0,
            channels: vec![simple_channel("command_power", "none", "stim")],
        },
        LslStream::FacialExpressions => OutletMeta {
            name: "EmotivFacialExpressions",
            stream_type: "Markers",
            srate: 0.0,
            channels: vec![
                simple_channel("upper_face_power", "none", "stim"),
                simple_channel("lower_face_power", "none", "stim"),
                simple_channel("reserved", "none", "stim"),
            ],
        },
        LslStream::DeviceQuality => {
            let mut channels = Vec::with_capacity(model.num_channels() + 3);
            for sensor in model.channel_names() {
                channels.push(simple_channel(
                    &format!("{}_contact_quality", sensor),
                    "none",
                    "misc",
                ));
            }
            channels.push(simple_channel("battery_percent", "%", "misc"));
            channels.push(simple_channel("signal_strength", "none", "misc"));
            channels.push(simple_channel("overall_quality", "none", "misc"));
            OutletMeta {
                name: "EmotivDeviceQuality",
                stream_type: "EEG",
                srate: 0.0,
                channels,
            }
        }
        LslStream::EegQuality => {
            let mut channels = Vec::with_capacity(model.num_channels() + 3);
            for sensor in model.channel_names() {
                channels.push(simple_channel(
                    &format!("{}_signal_quality", sensor),
                    "none",
                    "misc",
                ));
            }
            channels.push(simple_channel("battery_percent", "%", "misc"));
            channels.push(simple_channel("overall_quality", "none", "misc"));
            channels.push(simple_channel("sample_rate_quality", "none", "misc"));
            OutletMeta {
                name: "EmotivEEGQuality",
                stream_type: "EEG",
                srate: 0.0,
                channels,
            }
        }
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
        lsl::ChannelFormat::Float32,
        source_id,
    )?;

    let mut desc = info.desc();
    let mut channels = desc.append_child("channels");
    for ch in &meta.channels {
        let mut channel = channels.append_child("channel");
        channel = channel.append_child_value("label", &ch.label);
        channel = channel.append_child_value("unit", ch.unit);
        channel = channel.append_child_value("type", ch.kind);
        if let Some(location) = &ch.location {
            channel = channel.append_child_value("location", location);
        }
        let _ = channel;
    }

    let mut acquisition = desc.append_child("acquisition");
    acquisition = acquisition.append_child_value("manufacturer", "Emotiv");
    acquisition = acquisition.append_child_value("model", &model.to_string());
    let _ = acquisition;

    let mut source = desc.append_child("source");
    source = source.append_child_value("application", "emotiv-cortex-cli");
    source = source.append_child_value("library", "emotiv-cortex-v2");
    source = source.append_child_value("version", env!("CARGO_PKG_VERSION"));
    let _ = source;

    if meta.name == "EmotivEEG" {
        let mut reference = desc.append_child("reference");
        reference = reference.append_child_value("scheme", "unknown");
        reference = reference.append_child_value("notes", "not provided by Cortex eeg payload");
        let _ = reference;
    }

    Ok(info)
}

fn spawn_outlet_worker(
    meta: OutletMeta,
    source_id: String,
    model: HeadsetModel,
) -> Result<OutletWorker, Box<dyn std::error::Error>> {
    let (sample_tx, mut sample_rx) = mpsc::channel::<Vec<f32>>(1024);
    let (ready_tx, ready_rx) = std_mpsc::sync_channel::<Result<(), String>>(1);
    let thread_name = format!("lsl-outlet-{}", meta.name);

    let thread_handle = std::thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            let info = match build_stream_info(&meta, &source_id, &model) {
                Ok(info) => info,
                Err(err) => {
                    let _ = ready_tx.send(Err(err.to_string()));
                    return;
                }
            };

            let outlet = match lsl::StreamOutlet::new(&info, 0, 360) {
                Ok(outlet) => outlet,
                Err(err) => {
                    let _ = ready_tx.send(Err(format!("{err:?}")));
                    return;
                }
            };
            let _ = ready_tx.send(Ok(()));

            while let Some(sample) = sample_rx.blocking_recv() {
                if let Err(err) = outlet.push_sample(&sample) {
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
) -> Result<mpsc::Sender<Vec<f32>>, Box<dyn std::error::Error>> {
    active_outlets.push(format_outlet_summary(&meta).cyan().to_string());
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
}

impl LslStreamingHandle {
    /// Format a compact status string for display in the CLI status bar.
    ///
    /// Example: `LSL ▶ EEG, Motion`
    pub fn format_status(&self) -> String {
        let streams: Vec<&str> = self.subscribed.iter().map(|s| s.label()).collect();

        format!("LSL ▶ {}", streams.join(", "))
    }

    /// Format detailed per-stream statistics for the "View stats" sub-menu.
    ///
    /// Includes the outlet schema summary for each active stream.
    pub fn format_detailed_stats(&self) -> String {
        let elapsed = self.started_at.elapsed();
        let time_str = format_duration(elapsed);
        let mut out = format!(
            "{} Streaming for {}\n",
            "LSL Status:".green().bold(),
            time_str.cyan()
        );
        for (i, (name, count)) in self.sample_counts.iter().enumerate() {
            let n = count.load(Ordering::Relaxed);
            let outlet_name = self
                .active_streams
                .get(i)
                .map(|s| s.as_str())
                .unwrap_or("?");
            out.push_str(&format!(
                "  {} {:>12} samples  (outlet: {})\n",
                format!("{}:", name).cyan(),
                n,
                outlet_name,
            ));
        }
        out
    }

    /// Returns the list of currently subscribed Cortex stream types.
    pub fn subscribed_streams(&self) -> Vec<LslStream> {
        self.subscribed.clone()
    }
}

/// Format a duration into a human-readable string like "5m 23s" or "1h 2m 34s".
fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{}h {}m {}s", h, m, s)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
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
                                if sample_tx.send(data.channels).await.is_err() {
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
                                if sample_tx.send(sample).await.is_err() {
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
                                if sample_tx.send(sample).await.is_err() {
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
                                if sample_tx.send(sample).await.is_err() {
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
                let sample_tx = register_outlet(
                    &mut active_outlets,
                    &mut outlet_workers,
                    outlet_meta(LslStream::MentalCommands, model),
                    source_id,
                    model,
                )?;

                tasks.push(tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            item = stream.next() => {
                                let Some(data) = item else { break };
                                let sample = vec![data.power];
                                if sample_tx.send(sample).await.is_err() {
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
                let sample_tx = register_outlet(
                    &mut active_outlets,
                    &mut outlet_workers,
                    outlet_meta(LslStream::FacialExpressions, model),
                    source_id,
                    model,
                )?;

                tasks.push(tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            item = stream.next() => {
                                let Some(data) = item else { break };
                                let sample = vec![
                                    data.upper_face_power,
                                    data.lower_face_power,
                                    0.0, // placeholder
                                ];
                                if sample_tx.send(sample).await.is_err() {
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
                                if sample_tx.send(sample).await.is_err() {
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
                                sample.extend_from_slice(&data.sensor_quality);
                                sample.push(data.battery_percent as f32);
                                sample.push(data.overall);
                                sample.push(data.sample_rate_quality);
                                if sample_tx.send(sample).await.is_err() {
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

    println!(
        "{} {}",
        "LSL streaming active:".green().bold(),
        active_outlets.join(", "),
    );

    Ok(LslStreamingHandle {
        shutdown_tx,
        tasks,
        outlet_workers,
        sample_counts,
        started_at: Instant::now(),
        active_streams: active_outlets,
        subscribed: selected.to_vec(),
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
    println!("{}", "Shutting down LSL streaming...".yellow());
    let LslStreamingHandle {
        shutdown_tx,
        tasks,
        outlet_workers,
        sample_counts: _,
        started_at: _,
        active_streams: _,
        subscribed,
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
        eprintln!(
            "{}",
            "Warning: Some tasks did not shut down cleanly".yellow()
        );
    }

    // Drop worker senders and join outlet threads.
    for worker in outlet_workers {
        drop(worker.sample_tx);
        if worker.thread_handle.join().is_err() {
            eprintln!(
                "{}",
                "Warning: An LSL outlet thread panicked during shutdown".yellow()
            );
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
        eprintln!("{} {}", "Warning: Failed to unsubscribe:".yellow(), e);
    }

    println!("{}", "LSL streaming stopped.".green());
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
        assert!(xml.contains("<location>AF3</location>"));
        assert!(xml.contains("<unit>microvolts</unit>"));
        assert!(xml.contains("<type>eeg</type>"));
    }

    #[test]
    fn eeg_streaminfo_declares_reference_unknown() {
        let model = HeadsetModel::Insight;
        let meta = outlet_meta(LslStream::Eeg, &model);
        let info = build_stream_info(&meta, "INSIGHT-TEST", &model).unwrap();
        let xml = info.to_xml().unwrap();

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
    fn metrics_stream_type_is_empty() {
        let model = HeadsetModel::Insight;
        let meta = outlet_meta(LslStream::Metrics, &model);
        let info = build_stream_info(&meta, "MET-TEST", &model).unwrap();

        assert_eq!(meta.stream_type, "");
        assert_eq!(info.stream_type(), "");
    }

    #[test]
    fn markers_streams_use_stim_channel_type() {
        let model = HeadsetModel::Insight;
        for stream in [LslStream::MentalCommands, LslStream::FacialExpressions] {
            let meta = outlet_meta(stream, &model);
            let info = build_stream_info(&meta, "MARKER-TEST", &model).unwrap();
            let xml = info.to_xml().unwrap();

            assert!(meta.channels.iter().all(|c| c.kind == "stim"));
            assert_eq!(
                count_occurrences(&xml, "<type>stim</type>"),
                meta.channels.len()
            );
        }
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
            "EmotivMotion [type=MoCap, ch=10, srate=0Hz]"
        );
    }
}
