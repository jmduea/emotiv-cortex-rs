//! Application state for the TUI dashboard.
//!
//! [`App`] holds all mutable state consumed by the rendering and event-loop
//! layers: connection info, ring buffers for stream data, UI navigation,
//! and the optional LSL handle.

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

use emotiv_cortex_v2::headset::HeadsetModel;
use emotiv_cortex_v2::protocol::headset::HeadsetInfo;
use emotiv_cortex_v2::protocol::streams::{
    DeviceQuality, FacialExpression, MentalCommand, PerformanceMetrics,
};
use emotiv_cortex_v2::{CortexClient, CortexConfig};
use tokio::sync::mpsc;

use crate::event::{AppEvent, LogEntry};

/// Maximum number of samples kept per ring buffer channel.
const RING_BUFFER_CAP: usize = 256;

/// Maximum number of log entries retained.
const LOG_CAP: usize = 500;

// ─── Tab Enum ────────────────────────────────────────────────────────────

/// Top-level TUI tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    Dashboard,
    Streams,
    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    Lsl,
    Device,
    Log,
}

impl Tab {
    /// Ordered list used for tab bar rendering and keyboard navigation.
    pub fn all() -> &'static [Tab] {
        &[
            Tab::Dashboard,
            Tab::Streams,
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            Tab::Lsl,
            Tab::Device,
            Tab::Log,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Dashboard => "Dashboard",
            Tab::Streams => "Streams",
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            Tab::Lsl => "LSL",
            Tab::Device => "Device",
            Tab::Log => "Log",
        }
    }

    pub fn next(self) -> Self {
        let tabs = Self::all();
        let idx = tabs.iter().position(|&t| t == self).unwrap_or(0);
        tabs[(idx + 1) % tabs.len()]
    }

    pub fn prev(self) -> Self {
        let tabs = Self::all();
        let idx = tabs.iter().position(|&t| t == self).unwrap_or(0);
        tabs[(idx + tabs.len() - 1) % tabs.len()]
    }
}

// ─── Stream view selector (for Streams tab) ─────────────────────────────

/// Which stream is displayed in the Streams tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamView {
    Eeg,
    Motion,
    BandPower,
}

impl StreamView {
    pub fn label(self) -> &'static str {
        match self {
            StreamView::Eeg => "EEG",
            StreamView::Motion => "Motion",
            StreamView::BandPower => "Band Power",
        }
    }

    pub fn all() -> &'static [StreamView] {
        #![allow(dead_code)]
        &[StreamView::Eeg, StreamView::Motion, StreamView::BandPower]
    }

    pub fn next(self) -> Self {
        match self {
            StreamView::Eeg => StreamView::Motion,
            StreamView::Motion => StreamView::BandPower,
            StreamView::BandPower => StreamView::Eeg,
        }
    }
}

// ─── Connection phase (for startup flow) ─────────────────────────────────

/// Tracks where we are in the connection lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ConnectionPhase {
    /// Authenticating with the Cortex API.
    Authenticating,
    /// Authenticated — headsets discovered, waiting for user to pick one.
    Discovered,
    /// User selected a headset — connecting + creating session.
    ConnectingHeadset,
    /// Session active, streams subscribed.
    Ready,
    /// Something went wrong.
    Error,
}

impl ConnectionPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Authenticating => "Authenticating…",
            Self::Discovered => "Select a headset",
            Self::ConnectingHeadset => "Connecting to headset…",
            Self::Ready => "Ready",
            Self::Error => "Error",
        }
    }
}

// ─── Subscribed stream tracking ──────────────────────────────────────────

/// Which Cortex streams we have active subscriptions on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum StreamType {
    Eeg,
    Dev,
    Motion,
    BandPower,
    Metrics,
    MentalCommands,
    FacialExpressions,
    EegQuality,
}

// ─── Main App State ──────────────────────────────────────────────────────

/// All mutable TUI state.
#[allow(dead_code)]
pub struct App {
    // ── Connection ───────────────────────────────────────────────────
    pub client: Arc<CortexClient>,
    pub config: CortexConfig,
    pub token: Option<String>,
    pub session_id: Option<String>,
    pub headset_id: Option<String>,
    pub headset_info: Option<HeadsetInfo>,
    pub headset_model: Option<HeadsetModel>,
    pub phase: ConnectionPhase,
    // ── Device discovery ─────────────────────────────────────────────
    pub discovered_headsets: Vec<HeadsetInfo>,
    pub selected_headset_idx: usize,
    // ── Event channel (for spawning async work from key handlers) ──
    tx: mpsc::UnboundedSender<AppEvent>,
    /// Shutdown broadcast — shared with stream subscriber tasks.
    pub shutdown_tx: tokio::sync::broadcast::Sender<()>,
    // ── UI navigation ───────────────────────────────────────────────
    pub active_tab: Tab,
    pub stream_view: StreamView,
    pub scroll_offset: u16,
    pub show_help: bool,
    pub should_quit: bool,

    // ── Stream ring buffers ─────────────────────────────────────────
    pub eeg_buffers: Vec<VecDeque<f64>>,
    pub motion_accel: VecDeque<[f32; 3]>,
    pub motion_mag: VecDeque<[f32; 3]>,
    pub band_power_buffers: Vec<VecDeque<[f32; 5]>>,

    // ── Latest snapshot values ───────────────────────────────────────
    pub metrics: Option<PerformanceMetrics>,
    pub device_quality: Option<DeviceQuality>,
    pub mental_command: Option<MentalCommand>,
    pub facial_expression: Option<FacialExpression>,

    // ── Subscriptions ───────────────────────────────────────────────
    pub subscribed_streams: HashSet<StreamType>,

    // ── LSL ─────────────────────────────────────────────────────────
    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    pub lsl_streaming: Option<crate::lsl::LslStreamingHandle>,
    /// Streams selected for LSL publication (configured before starting).
    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    pub lsl_selected_streams: std::collections::HashSet<crate::lsl::LslStream>,
    /// Cursor row in the stream-selection checklist (0-based).
    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    pub lsl_cursor: usize,
    /// Index of the stream whose XML is shown in the XML viewer.
    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    pub lsl_xml_stream_idx: usize,
    /// Vertical scroll offset for the XML viewer paragraph.
    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    pub lsl_xml_scroll: u16,
    /// Whether the XML metadata panel is visible.
    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    pub lsl_show_xml: bool,

    // ── Log ─────────────────────────────────────────────────────────
    pub log_entries: VecDeque<LogEntry>,
    pub log_auto_scroll: bool,

    // ── Timing ──────────────────────────────────────────────────────
    pub started_at: std::time::Instant,
}

impl App {
    /// Create a new `App` with default (empty) state.
    pub fn new(
        client: Arc<CortexClient>,
        config: CortexConfig,
        tx: mpsc::UnboundedSender<AppEvent>,
        shutdown_tx: tokio::sync::broadcast::Sender<()>,
    ) -> Self {
        Self {
            client,
            config,
            token: None,
            session_id: None,
            headset_id: None,
            headset_info: None,
            headset_model: None,
            phase: ConnectionPhase::Authenticating,

            discovered_headsets: Vec::new(),
            selected_headset_idx: 0,

            tx,
            shutdown_tx,

            active_tab: Tab::Dashboard,
            stream_view: StreamView::Eeg,
            scroll_offset: 0,
            show_help: false,
            should_quit: false,

            eeg_buffers: Vec::new(),
            motion_accel: VecDeque::with_capacity(RING_BUFFER_CAP),
            motion_mag: VecDeque::with_capacity(RING_BUFFER_CAP),
            band_power_buffers: Vec::new(),

            metrics: None,
            device_quality: None,
            mental_command: None,
            facial_expression: None,

            subscribed_streams: HashSet::new(),

            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            lsl_streaming: None,
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            lsl_selected_streams: crate::lsl::LslStream::all().iter().copied().collect(),
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            lsl_cursor: 0,
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            lsl_xml_stream_idx: 0,
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            lsl_xml_scroll: 0,
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            lsl_show_xml: false,

            log_entries: VecDeque::with_capacity(LOG_CAP),
            log_auto_scroll: true,

            started_at: std::time::Instant::now(),
        }
    }

    /// Initialize EEG ring buffers for the given channel count.
    pub fn init_eeg_buffers(&mut self, num_channels: usize) {
        self.eeg_buffers = (0..num_channels)
            .map(|_| VecDeque::with_capacity(RING_BUFFER_CAP))
            .collect();
    }

    /// Initialize band-power ring buffers for the given channel count.
    pub fn init_band_power_buffers(&mut self, num_channels: usize) {
        self.band_power_buffers = (0..num_channels)
            .map(|_| VecDeque::with_capacity(RING_BUFFER_CAP))
            .collect();
    }

    /// Push a log entry, evicting the oldest if at capacity.
    pub fn log(&mut self, entry: LogEntry) {
        if self.log_entries.len() >= LOG_CAP {
            self.log_entries.pop_front();
        }
        self.log_entries.push_back(entry);
    }

    /// Process an incoming [`AppEvent`], updating state accordingly.
    ///
    /// Returns `true` if the app should quit.
    pub fn handle_event(&mut self, event: AppEvent) -> bool {
        match event {
            AppEvent::Terminal(crossterm::event::Event::Key(key))
                if key.kind == crossterm::event::KeyEventKind::Press =>
            {
                self.handle_key(key);
            }
            AppEvent::Eeg(ref data) => self.push_eeg(data),
            AppEvent::DeviceQuality(dq) => self.device_quality = Some(dq),
            AppEvent::Motion(ref m) => self.push_motion(m),
            AppEvent::BandPower(ref bp) => self.push_band_power(bp),
            AppEvent::Metrics(m) => self.metrics = Some(m),
            AppEvent::MentalCommand(mc) => self.mental_command = Some(mc),
            AppEvent::FacialExpression(fe) => self.facial_expression = Some(fe),
            AppEvent::EegQuality(_eq) => { /* stored as part of DeviceQuality for now */ }
            AppEvent::HeadsetUpdate(headsets) => {
                self.discovered_headsets = headsets;
                // Clamp selection index
                if self.discovered_headsets.is_empty() {
                    self.selected_headset_idx = 0;
                } else {
                    self.selected_headset_idx = self
                        .selected_headset_idx
                        .min(self.discovered_headsets.len() - 1);
                }
                // Update headset_info if we're already connected
                if let Some(ref hid) = self.headset_id {
                    if let Some(h) = self.discovered_headsets.iter().find(|h| &h.id == hid) {
                        self.headset_info = Some(h.clone());
                    }
                }
            }
            AppEvent::AuthReady { token } => {
                self.token = Some(token);
                self.phase = ConnectionPhase::Discovered;
                self.log(LogEntry::info(
                    "Authenticated — select a headset in the Device tab",
                ));
                // Auto-switch to Device tab so the user sees the headset list
                self.active_tab = Tab::Device;
            }
            AppEvent::ConnectionReady {
                token,
                session_id,
                headset_id,
                model,
            } => {
                self.token = Some(token);
                self.session_id = Some(session_id);
                // Populate headset_info immediately from the already-fetched
                // discovery list so the Device tab renders without needing a
                // manual 'r' refresh.
                self.headset_info = self
                    .discovered_headsets
                    .iter()
                    .find(|h| h.id == headset_id)
                    .cloned();
                self.headset_id = Some(headset_id);
                self.headset_model = Some(model);
                self.phase = ConnectionPhase::Ready;
                self.log(LogEntry::info("Connection ready"));
            }
            AppEvent::StreamsSubscribed(streams) => {
                self.subscribed_streams = streams.into_iter().collect();
            }
            AppEvent::ConnectionFailed => {
                self.phase = ConnectionPhase::Discovered;
                self.log(LogEntry::warn(
                    "Connection failed — select a headset and try again",
                ));
            }
            AppEvent::Disconnected => {
                // Signal existing stream tasks to stop
                let _ = self.shutdown_tx.send(());
                // Create a fresh shutdown channel for the next connection
                let (new_tx, _) = tokio::sync::broadcast::channel::<()>(1);
                self.shutdown_tx = new_tx;

                self.session_id = None;
                self.headset_id = None;
                self.headset_info = None;
                self.headset_model = None;
                self.phase = ConnectionPhase::Discovered;
                self.subscribed_streams.clear();
                self.device_quality = None;
                self.metrics = None;
                self.mental_command = None;
                self.facial_expression = None;
                self.eeg_buffers.clear();
                self.motion_accel.clear();
                self.motion_mag.clear();
                self.band_power_buffers.clear();
                self.log(LogEntry::info(
                    "Disconnected — select a headset to reconnect",
                ));
                self.active_tab = Tab::Device;
                // Refresh headset list so user sees current state
                self.refresh_headsets();
            }
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            AppEvent::LslStarted(handle) => {
                self.log(LogEntry::info("LSL streaming started"));
                self.lsl_streaming = Some(handle);
            }
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            AppEvent::LslStopped => {
                self.log(LogEntry::info("LSL streaming stopped"));
                self.lsl_streaming = None;
                self.lsl_show_xml = false;
                self.lsl_xml_stream_idx = 0;
                self.lsl_xml_scroll = 0;
            }
            AppEvent::Log(entry) => self.log(entry),
            AppEvent::Quit => self.should_quit = true,
            AppEvent::Tick | AppEvent::Terminal(_) => {}
        }
        self.should_quit
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::{KeyCode, KeyModifiers};

        // Global: Ctrl+C quits
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }
        if key.code == KeyCode::Char('q') && !self.show_help {
            self.should_quit = true;
            return;
        }

        match key.code {
            // Help overlay toggle
            KeyCode::Char('?') => self.show_help = !self.show_help,

            // Tab switching by number
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                let tabs = Tab::all();
                if idx < tabs.len() {
                    self.active_tab = tabs[idx];
                    self.scroll_offset = 0;
                }
            }

            // Tab switching by Tab/Shift+Tab
            KeyCode::Tab => {
                self.active_tab = self.active_tab.next();
                self.scroll_offset = 0;
            }
            KeyCode::BackTab => {
                self.active_tab = self.active_tab.prev();
                self.scroll_offset = 0;
            }

            // Scrolling
            KeyCode::Up | KeyCode::Char('k') => {
                if self.active_tab == Tab::Device && self.phase == ConnectionPhase::Discovered {
                    self.selected_headset_idx = self.selected_headset_idx.saturating_sub(1);
                } else {
                    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
                    if self.active_tab == Tab::Lsl {
                        if self.lsl_streaming.is_some() && self.lsl_show_xml {
                            self.lsl_xml_scroll = self.lsl_xml_scroll.saturating_sub(1);
                        } else if self.lsl_streaming.is_none() {
                            self.lsl_cursor = self.lsl_cursor.saturating_sub(1);
                        }
                        return;
                    }
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.active_tab == Tab::Device && self.phase == ConnectionPhase::Discovered {
                    let max = self.discovered_headsets.len().saturating_sub(1);
                    self.selected_headset_idx =
                        self.selected_headset_idx.saturating_add(1).min(max);
                } else {
                    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
                    if self.active_tab == Tab::Lsl {
                        if self.lsl_show_xml {
                            if let Some(ref handle) = self.lsl_streaming {
                                let idx = self
                                    .lsl_xml_stream_idx
                                    .min(handle.stream_xml_metadata.len().saturating_sub(1));
                                let line_count = handle
                                    .stream_xml_metadata
                                    .get(idx)
                                    .map_or(0, |(_, x)| x.lines().count());
                                self.lsl_xml_scroll = self.lsl_xml_scroll.saturating_add(1).min(
                                    u16::try_from(line_count.saturating_sub(1)).unwrap_or(u16::MAX),
                                );
                            }
                        } else if self.lsl_streaming.is_none() {
                            let max = crate::lsl::LslStream::all().len().saturating_sub(1);
                            self.lsl_cursor = self.lsl_cursor.saturating_add(1).min(max);
                        }
                        return;
                    }
                    self.scroll_offset = self.scroll_offset.saturating_add(1);
                }
            }

            // Device tab: connect to selected headset
            KeyCode::Enter if self.active_tab == Tab::Device => {
                self.connect_selected_headset();
            }

            // Device tab: disconnect from headset
            KeyCode::Char('d') if self.active_tab == Tab::Device => {
                self.disconnect_headset();
            }

            // Device tab: refresh headset list
            KeyCode::Char('r') if self.active_tab == Tab::Device => {
                self.refresh_headsets();
            }

            // Stream view cycling (on Streams tab)
            KeyCode::Char('v') if self.active_tab == Tab::Streams => {
                self.stream_view = self.stream_view.next();
            }

            // LSL toggle (on LSL tab)
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            KeyCode::Char('l') if self.active_tab == Tab::Lsl => {
                self.toggle_lsl();
            }

            // LSL: Enter starts streaming when inactive (mirrors 'l')
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            KeyCode::Enter if self.active_tab == Tab::Lsl && self.lsl_streaming.is_none() => {
                self.toggle_lsl();
            }

            // LSL stream selection: Space toggles the highlighted stream
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            KeyCode::Char(' ') if self.active_tab == Tab::Lsl && self.lsl_streaming.is_none() => {
                if let Some(&stream) = crate::lsl::LslStream::all().get(self.lsl_cursor) {
                    if self.lsl_selected_streams.contains(&stream) {
                        self.lsl_selected_streams.remove(&stream);
                    } else {
                        self.lsl_selected_streams.insert(stream);
                    }
                }
            }

            // LSL stream selection: 'a' selects all, 'n' clears selection
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            KeyCode::Char('a') if self.active_tab == Tab::Lsl && self.lsl_streaming.is_none() => {
                self.lsl_selected_streams = crate::lsl::LslStream::all().iter().copied().collect();
            }
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            KeyCode::Char('n') if self.active_tab == Tab::Lsl && self.lsl_streaming.is_none() => {
                self.lsl_selected_streams.clear();
            }

            // LSL XML viewer: 'x' toggles, Left/Right cycles between streams
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            KeyCode::Char('x') if self.active_tab == Tab::Lsl && self.lsl_streaming.is_some() => {
                self.lsl_show_xml = !self.lsl_show_xml;
                self.lsl_xml_scroll = 0;
            }
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            KeyCode::Left
                if self.active_tab == Tab::Lsl
                    && self.lsl_show_xml
                    && self.lsl_streaming.is_some() =>
            {
                let len = self
                    .lsl_streaming
                    .as_ref()
                    .map_or(0, |h| h.stream_xml_metadata.len());
                if len > 0 {
                    self.lsl_xml_stream_idx = (self.lsl_xml_stream_idx + len - 1) % len;
                    self.lsl_xml_scroll = 0;
                }
            }
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            KeyCode::Right
                if self.active_tab == Tab::Lsl
                    && self.lsl_show_xml
                    && self.lsl_streaming.is_some() =>
            {
                let len = self
                    .lsl_streaming
                    .as_ref()
                    .map_or(0, |h| h.stream_xml_metadata.len());
                if len > 0 {
                    self.lsl_xml_stream_idx = (self.lsl_xml_stream_idx + 1) % len;
                    self.lsl_xml_scroll = 0;
                }
            }

            _ => {}
        }
    }

    // ── Ring buffer pushers ──────────────────────────────────────────

    fn push_eeg(&mut self, data: &emotiv_cortex_v2::protocol::streams::EegData) {
        if self.eeg_buffers.is_empty() && !data.channels.is_empty() {
            self.init_eeg_buffers(data.channels.len());
        }
        for (i, &val) in data.channels.iter().enumerate() {
            if let Some(buf) = self.eeg_buffers.get_mut(i) {
                if buf.len() >= RING_BUFFER_CAP {
                    buf.pop_front();
                }
                buf.push_back(f64::from(val));
            }
        }
    }

    fn push_motion(&mut self, data: &emotiv_cortex_v2::protocol::streams::MotionData) {
        if self.motion_accel.len() >= RING_BUFFER_CAP {
            self.motion_accel.pop_front();
        }
        self.motion_accel.push_back(data.accelerometer);

        if self.motion_mag.len() >= RING_BUFFER_CAP {
            self.motion_mag.pop_front();
        }
        self.motion_mag.push_back(data.magnetometer);
    }

    fn push_band_power(&mut self, data: &emotiv_cortex_v2::protocol::streams::BandPowerData) {
        if self.band_power_buffers.is_empty() && !data.channel_powers.is_empty() {
            self.init_band_power_buffers(data.channel_powers.len());
        }
        for (i, &powers) in data.channel_powers.iter().enumerate() {
            if let Some(buf) = self.band_power_buffers.get_mut(i) {
                if buf.len() >= RING_BUFFER_CAP {
                    buf.pop_front();
                }
                buf.push_back(powers);
            }
        }
    }

    /// Elapsed time since the app started.
    pub fn uptime(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    // ── Device connection ───────────────────────────────────────────

    /// Connect to the currently selected headset in the Device tab.
    ///
    /// Spawns a background task that connects the headset, creates a
    /// session, then subscribes to default streams.
    fn connect_selected_headset(&mut self) {
        if self.phase != ConnectionPhase::Discovered {
            self.log(LogEntry::warn("Already connected or not yet authenticated"));
            return;
        }

        let Some(headset) = self
            .discovered_headsets
            .get(self.selected_headset_idx)
            .cloned()
        else {
            self.log(LogEntry::warn("No headset selected"));
            return;
        };

        self.phase = ConnectionPhase::ConnectingHeadset;

        let client = Arc::clone(&self.client);
        let token = self.token.clone().unwrap_or_default();
        let tx = self.tx.clone();
        let shutdown = self.shutdown_tx.clone();

        tokio::spawn(async move {
            match crate::bridge::connect_headset_and_create_session(&client, &token, &headset, &tx)
                .await
            {
                Ok(result) => {
                    let _ = tx.send(AppEvent::ConnectionReady {
                        token: token.clone(),
                        session_id: result.session_id.clone(),
                        headset_id: result.headset_id.clone(),
                        model: result.model.clone(),
                    });

                    match crate::bridge::subscribe_default_streams(
                        &client,
                        &token,
                        &result.session_id,
                        &result.model,
                        tx.clone(),
                        shutdown,
                    )
                    .await
                    {
                        Ok(streams) => {
                            let _ = tx.send(AppEvent::StreamsSubscribed(streams));
                        }
                        Err(e) => {
                            let _ = tx.send(AppEvent::Log(LogEntry::error(format!(
                                "Stream subscription failed: {e}"
                            ))));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::Log(LogEntry::error(format!(
                        "Connection failed: {e}"
                    ))));
                    let _ = tx.send(AppEvent::ConnectionFailed);
                }
            }
        });
    }

    /// Disconnect from the current headset, closing the session.
    ///
    /// Spawns a background task that closes the session and optionally
    /// disconnects the headset at the Bluetooth level.
    fn disconnect_headset(&mut self) {
        if self.phase != ConnectionPhase::Ready {
            self.log(LogEntry::warn("Not connected — nothing to disconnect"));
            return;
        }

        let Some(token) = self.token.clone() else {
            self.log(LogEntry::warn("No token available"));
            return;
        };
        let Some(session_id) = self.session_id.clone() else {
            self.log(LogEntry::warn("No active session"));
            return;
        };
        let headset_id = self.headset_id.clone();

        // Stop LSL before disconnecting
        #[cfg(all(feature = "lsl", not(target_os = "linux")))]
        if let Some(handle) = self.lsl_streaming.take() {
            let client = Arc::clone(&self.client);
            let t = token.clone();
            let s = session_id.clone();
            tokio::spawn(async move {
                let _ = crate::lsl::stop_lsl_streaming(handle, &client, &t, &s).await;
            });
        }

        self.phase = ConnectionPhase::ConnectingHeadset; // reuse as "busy" indicator
        self.log(LogEntry::info("Disconnecting…"));

        let client = Arc::clone(&self.client);
        let tx = self.tx.clone();

        tokio::spawn(async move {
            match crate::bridge::disconnect_and_close_session(
                &client,
                &token,
                &session_id,
                headset_id.as_deref(),
                &tx,
            )
            .await
            {
                Ok(()) => {
                    let _ = tx.send(AppEvent::Disconnected);
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::Log(LogEntry::error(format!(
                        "Disconnect failed: {e}"
                    ))));
                    // Still transition back so the user isn't stuck
                    let _ = tx.send(AppEvent::Disconnected);
                }
            }
        });
    }

    /// Re-query available headsets.
    fn refresh_headsets(&mut self) {
        let client = Arc::clone(&self.client);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::bridge::refresh_headsets(&client, &tx).await {
                let _ = tx.send(AppEvent::Log(LogEntry::error(format!(
                    "Refresh failed: {e}"
                ))));
            }
        });
    }

    // ── LSL toggle ───────────────────────────────────────────────────

    /// Start or stop LSL streaming.
    ///
    /// Because `start_lsl_streaming` / `stop_lsl_streaming` are async, we
    /// spawn a background tokio task and send the result back via the
    /// event channel.
    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    fn toggle_lsl(&mut self) {
        if self.lsl_streaming.is_some() {
            // Stop
            let handle = self.lsl_streaming.take().expect("checked is_some above");
            let client = Arc::clone(&self.client);
            let token = self.token.clone().unwrap_or_default();
            let session_id = self.session_id.clone().unwrap_or_default();
            let tx = self.tx.clone();
            self.log(LogEntry::info("Stopping LSL streaming…"));
            tokio::spawn(async move {
                match crate::lsl::stop_lsl_streaming(handle, &client, &token, &session_id).await {
                    Ok(()) => {
                        let _ = tx.send(AppEvent::LslStopped);
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::Log(LogEntry::error(format!(
                            "LSL stop failed: {e}"
                        ))));
                    }
                }
            });
        } else if self.phase == ConnectionPhase::Ready {
            // Start — use the user-configured selection in LslStream::all() order
            let selected: Vec<crate::lsl::LslStream> = crate::lsl::LslStream::all()
                .iter()
                .filter(|s| self.lsl_selected_streams.contains(s))
                .copied()
                .collect();
            if selected.is_empty() {
                self.log(LogEntry::warn(
                    "No streams selected — pick at least one with Space",
                ));
                return;
            }
            // Reset XML viewer for the new session
            self.lsl_xml_stream_idx = 0;
            self.lsl_xml_scroll = 0;
            self.lsl_show_xml = false;

            let client = Arc::clone(&self.client);
            let token = self.token.clone().unwrap_or_default();
            let session_id = self.session_id.clone().unwrap_or_default();
            let model = self
                .headset_model
                .clone()
                .unwrap_or(emotiv_cortex_v2::headset::HeadsetModel::Insight);
            let source_id = self
                .headset_id
                .clone()
                .unwrap_or_else(|| "emotiv-unknown".to_string());
            let tx = self.tx.clone();
            self.log(LogEntry::info("Starting LSL streaming…"));
            tokio::spawn(async move {
                match crate::lsl::start_lsl_streaming(
                    &client,
                    &token,
                    &session_id,
                    &model,
                    &selected,
                    &source_id,
                )
                .await
                {
                    Ok(handle) => {
                        let _ = tx.send(AppEvent::LslStarted(handle));
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::Log(LogEntry::error(format!(
                            "LSL start failed: {e}"
                        ))));
                    }
                }
            });
        } else {
            self.log(LogEntry::warn("Cannot start LSL — not yet connected"));
        }
    }
}
