//! # Stream Utilities
//!
//! Typed stream adapters for Cortex data streams.
//!
//! ## `TypedStream`
//!
//! [`TypedStream`] is a generic adapter that converts raw JSON events from an
//! `mpsc` channel into typed values using a parser closure. Events that fail
//! to parse are silently skipped.
//!
//! ## Convenience Subscriptions
//!
//! This module provides subscribe functions for all 9 Cortex data streams.
//! Each function:
//! 1. Creates an mpsc channel on the client
//! 2. Sends the `subscribe` RPC call
//! 3. Returns a typed `Stream` that yields parsed data
//!
//! ```no_run
//! use emotiv_cortex_v2::streams;
//! use emotiv_cortex_v2::CortexClient;
//!
//! # async fn demo(client: &CortexClient, token: &str, session_id: &str) -> emotiv_cortex_v2::CortexResult<()> {
//! let eeg = streams::subscribe_eeg(client, token, session_id, 5).await?;
//! let mot = streams::subscribe_motion(client, token, session_id).await?;
//! let _ = (eeg, mot);
//! # Ok(())
//! # }
//! ```

use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Stream;
use tokio::sync::mpsc;

use crate::client::CortexClient;
use crate::error::{CortexError, CortexResult};
use crate::protocol::constants::Streams;
use crate::protocol::streams::{
    BandPowerData, DeviceQuality, EegData, EegQuality, EqEvent, FacialExpression, MentalCommand,
    MotEvent, MotionData, PerformanceMetrics, PowEvent, SysEvent,
};

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

/// Generic stream adapter that receives raw JSON events from an mpsc channel
/// and transforms them into typed values using a parser closure.
///
/// Events that fail to parse are silently skipped (they may be malformed
/// or from an incompatible Cortex API version).
///
/// # Example
///
/// ```rust
/// use emotiv_cortex_v2::streams::TypedStream;
/// use futures_util::StreamExt;
/// use tokio::sync::mpsc;
///
/// let rt = tokio::runtime::Builder::new_current_thread()
///     .enable_time()
///     .build()
///     .unwrap();
///
/// rt.block_on(async {
///     let (tx, rx) = mpsc::channel(4);
///     let mut stream = TypedStream::new(rx, |event| {
///         event.get("value")?.as_i64().map(|v| v as i32)
///     });
///
///     tx.send(serde_json::json!({"value": 7})).await.unwrap();
///     drop(tx);
///
///     assert_eq!(stream.next().await, Some(7));
/// });
/// ```
pub struct TypedStream<T, F>
where
    F: Fn(serde_json::Value) -> Option<T>,
{
    rx: mpsc::Receiver<serde_json::Value>,
    parser: F,
}

impl<T, F> TypedStream<T, F>
where
    F: Fn(serde_json::Value) -> Option<T>,
{
    /// Create a new typed stream from a receiver and a parser function.
    pub fn new(rx: mpsc::Receiver<serde_json::Value>, parser: F) -> Self {
        Self { rx, parser }
    }
}

impl<T, F> Stream for TypedStream<T, F>
where
    T: Send,
    F: Fn(serde_json::Value) -> Option<T> + Unpin + Send,
{
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match self.rx.poll_recv(cx) {
                Poll::Ready(Some(event)) => {
                    if let Some(parsed) = (self.parser)(event) {
                        return Poll::Ready(Some(parsed));
                    }
                    // Parse failed — skip and try the next event
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

// ─── Helper ──────────────────────────────────────────────────────────────

/// Create a stream channel on the client, returning a `ProtocolError` if the
/// internal mutex is poisoned (should never happen in practice).
fn add_channel(
    client: &CortexClient,
    stream: &str,
) -> CortexResult<mpsc::Receiver<serde_json::Value>> {
    client
        .add_stream_channel(stream)
        .ok_or_else(|| CortexError::ProtocolError {
            reason: format!("Failed to create {stream} stream channel"),
        })
}

// ─── EEG Stream ──────────────────────────────────────────────────────────

/// Subscribe to the raw EEG data stream.
///
/// Returns a stream of [`EegData`] containing parsed per-sample EEG channel
/// values, sample counter, interpolation flag, and contact quality.
///
/// `num_channels` must match the headset's EEG channel count (e.g. 5 for
/// Insight, 14 for EPOC+/EPOC X). Use [`HeadsetModel::num_channels()`]
/// to get this value.
///
/// [`HeadsetModel::num_channels()`]: crate::headset::HeadsetModel::num_channels
///
/// # Examples
///
/// ```no_run
/// use futures_util::StreamExt;
/// use emotiv_cortex_v2::{CortexClient, CortexConfig, streams};
/// use emotiv_cortex_v2::headset::HeadsetModel;
///
/// # async fn demo() -> emotiv_cortex_v2::CortexResult<()> {
/// let config = CortexConfig::discover(None)?;
/// let mut client = CortexClient::connect(&config).await?;
/// let token = client.authenticate(&config.client_id, &config.client_secret).await?;
///
/// let session = client.create_session(&token, "INSIGHT-12345678").await?;
/// let mut eeg = streams::subscribe_eeg(
///     &client, &token, &session.id, HeadsetModel::Insight.num_channels(),
/// ).await?;
///
/// while let Some(sample) = eeg.next().await {
///     println!("EEG sample #{}: {:?}", sample.counter, sample.channels);
/// }
/// # Ok(())
/// # }
/// ```
///
/// # Errors
/// Returns any error produced by stream channel registration or
/// subscription RPC calls.
pub async fn subscribe_eeg(
    client: &CortexClient,
    cortex_token: &str,
    session_id: &str,
    num_channels: usize,
) -> CortexResult<Pin<Box<dyn Stream<Item = EegData> + Send>>> {
    let rx = add_channel(client, Streams::EEG)?;

    client
        .subscribe_streams(cortex_token, session_id, &[Streams::EEG])
        .await?;

    Ok(Box::pin(TypedStream::new(rx, move |event| {
        let time = event.get("time")?.as_f64()?;
        let eeg_array = event.get("eeg")?.as_array()?;
        EegData::from_eeg_array(eeg_array, num_channels, time)
    })))
}

// ─── Device Quality Stream ───────────────────────────────────────────────

/// Subscribe to the device quality stream.
///
/// Returns a stream of [`DeviceQuality`] containing battery level and
/// per-channel contact quality values.
///
/// `num_channels` must match the headset's EEG channel count (e.g. 5 for
/// Insight, 14 for EPOC+/EPOC X). Use [`HeadsetModel::num_channels()`]
/// to get this value.
///
/// [`HeadsetModel::num_channels()`]: crate::headset::HeadsetModel::num_channels
///
/// # Errors
/// Returns any error produced by stream channel registration or
/// subscription RPC calls.
pub async fn subscribe_dev(
    client: &CortexClient,
    cortex_token: &str,
    session_id: &str,
    num_channels: usize,
) -> CortexResult<Pin<Box<dyn Stream<Item = DeviceQuality> + Send>>> {
    let rx = add_channel(client, Streams::DEV)?;

    client
        .subscribe_streams(cortex_token, session_id, &[Streams::DEV])
        .await?;

    Ok(Box::pin(TypedStream::new(rx, move |event| {
        let dev_array = event.get("dev")?.as_array()?;
        let dev_values: Vec<serde_json::Value> = dev_array.clone();
        DeviceQuality::from_dev_array(&dev_values, num_channels)
    })))
}

// ─── Motion Stream ───────────────────────────────────────────────────────

/// Subscribe to the motion/IMU data stream.
///
/// Returns a stream of [`MotionData`] containing accelerometer,
/// magnetometer, and quaternion readings.
///
/// # Errors
/// Returns any error produced by stream channel registration or
/// subscription RPC calls.
pub async fn subscribe_motion(
    client: &CortexClient,
    cortex_token: &str,
    session_id: &str,
) -> CortexResult<Pin<Box<dyn Stream<Item = MotionData> + Send>>> {
    let rx = add_channel(client, Streams::MOT)?;

    client
        .subscribe_streams(cortex_token, session_id, &[Streams::MOT])
        .await?;

    Ok(Box::pin(TypedStream::new(rx, |event| {
        let mot_event: MotEvent = serde_json::from_value(event).ok()?;
        MotionData::from_mot_array(&mot_event.mot, mot_event.time)
    })))
}

// ─── EEG Quality Stream ─────────────────────────────────────────────────

/// Subscribe to the EEG quality stream.
///
/// Returns a stream of [`EegQuality`] containing per-channel signal
/// quality values. This is a higher-level quality metric than the raw
/// `dev` stream — values indicate signal quality rather than contact quality.
///
/// `num_channels` must match the headset's EEG channel count.
///
/// # Errors
/// Returns any error produced by stream channel registration or
/// subscription RPC calls.
pub async fn subscribe_eq(
    client: &CortexClient,
    cortex_token: &str,
    session_id: &str,
    num_channels: usize,
) -> CortexResult<Pin<Box<dyn Stream<Item = EegQuality> + Send>>> {
    let rx = add_channel(client, Streams::EQ)?;

    client
        .subscribe_streams(cortex_token, session_id, &[Streams::EQ])
        .await?;

    Ok(Box::pin(TypedStream::new(rx, move |event| {
        let eq_event: EqEvent = serde_json::from_value(event).ok()?;
        EegQuality::from_eq_array(&eq_event.eq, num_channels)
    })))
}

// ─── Band Power Stream ──────────────────────────────────────────────────

/// Subscribe to the band power stream.
///
/// Returns a stream of [`BandPowerData`] containing per-channel
/// frequency band powers (theta/alpha/betaL/betaH/gamma in uV^2/Hz).
///
/// `num_channels` must match the headset's EEG channel count.
///
/// # Errors
/// Returns any error produced by stream channel registration or
/// subscription RPC calls.
pub async fn subscribe_band_power(
    client: &CortexClient,
    cortex_token: &str,
    session_id: &str,
    num_channels: usize,
) -> CortexResult<Pin<Box<dyn Stream<Item = BandPowerData> + Send>>> {
    let rx = add_channel(client, Streams::POW)?;

    client
        .subscribe_streams(cortex_token, session_id, &[Streams::POW])
        .await?;

    Ok(Box::pin(TypedStream::new(rx, move |event| {
        let pow_event: PowEvent = serde_json::from_value(event).ok()?;
        BandPowerData::from_pow_array(&pow_event.pow, num_channels, pow_event.time)
    })))
}

// ─── Performance Metrics Stream ─────────────────────────────────────────

/// Subscribe to the performance metrics stream.
///
/// Returns a stream of [`PerformanceMetrics`] containing Emotiv's
/// computed cognitive state metrics (engagement, stress, attention, etc.).
///
/// # Errors
/// Returns any error produced by stream channel registration or
/// subscription RPC calls.
pub async fn subscribe_metrics(
    client: &CortexClient,
    cortex_token: &str,
    session_id: &str,
) -> CortexResult<Pin<Box<dyn Stream<Item = PerformanceMetrics> + Send>>> {
    let rx = add_channel(client, Streams::MET)?;

    let resp = client
        .subscribe_streams(cortex_token, session_id, &[Streams::MET])
        .await?;

    let cols: Vec<String> = resp
        .get("success")
        .and_then(|s| s.as_array())
        .and_then(|a| a.first())
        .and_then(|entry| entry.get("cols"))
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let col_idx = |name: &str| cols.iter().position(|c| c == name);
    let att_idx = col_idx("attention");
    let eng_idx = col_idx("eng");
    let exc_idx = col_idx("exc");
    let lex_idx = col_idx("lex");
    let str_idx = col_idx("str");
    let rel_idx = col_idx("rel");
    let int_idx = col_idx("int");
    let foc_idx = col_idx("foc");

    Ok(Box::pin(TypedStream::new(rx, move |event| {
        let met = event.get("met")?.as_array()?;
        let val = |i: usize| -> Option<f32> {
            met.get(i)
                .and_then(serde_json::Value::as_f64)
                .and_then(f64_to_f32)
        };
        let time = event.get("time")?.as_f64()?;
        Some(PerformanceMetrics {
            timestamp: seconds_to_micros_i64(time)?,
            attention: att_idx.and_then(&val),
            engagement: eng_idx.and_then(&val),
            excitement: exc_idx.and_then(&val),
            long_excitement: lex_idx.and_then(&val),
            stress: str_idx.and_then(&val),
            relaxation: rel_idx.and_then(&val),
            interest: int_idx.and_then(&val),
            focus: foc_idx.and_then(&val),
        })
    })))
}

// ─── Mental Command Stream ──────────────────────────────────────────────

/// Subscribe to the mental command stream.
///
/// Returns a stream of [`MentalCommand`] with the detected action and power.
/// Requires a loaded profile with trained mental commands.
///
/// # Errors
/// Returns any error produced by stream channel registration or
/// subscription RPC calls.
pub async fn subscribe_mental_commands(
    client: &CortexClient,
    cortex_token: &str,
    session_id: &str,
) -> CortexResult<Pin<Box<dyn Stream<Item = MentalCommand> + Send>>> {
    let rx = add_channel(client, Streams::COM)?;

    client
        .subscribe_streams(cortex_token, session_id, &[Streams::COM])
        .await?;

    Ok(Box::pin(TypedStream::new(rx, |event| {
        let com = event.get("com")?.as_array()?;
        let action = com.first()?.as_str()?.to_string();
        let power = f64_to_f32(com.get(1)?.as_f64()?)?;
        Some(MentalCommand { action, power })
    })))
}

// ─── Facial Expression Stream ───────────────────────────────────────────

/// Subscribe to the facial expression stream.
///
/// Returns a stream of [`FacialExpression`] with eye actions,
/// upper/lower face actions and their power levels.
///
/// # Errors
/// Returns any error produced by stream channel registration or
/// subscription RPC calls.
pub async fn subscribe_facial_expressions(
    client: &CortexClient,
    cortex_token: &str,
    session_id: &str,
) -> CortexResult<Pin<Box<dyn Stream<Item = FacialExpression> + Send>>> {
    let rx = add_channel(client, Streams::FAC)?;

    client
        .subscribe_streams(cortex_token, session_id, &[Streams::FAC])
        .await?;

    Ok(Box::pin(TypedStream::new(rx, |event| {
        let fac = event.get("fac")?.as_array()?;
        let eye_action = fac.first()?.as_str()?.to_string();
        let upper_face_action = fac.get(1)?.as_str()?.to_string();
        let upper_face_power = f64_to_f32(fac.get(2)?.as_f64()?)?;
        let lower_face_action = fac.get(3)?.as_str()?.to_string();
        let lower_face_power = f64_to_f32(fac.get(4)?.as_f64()?)?;
        Some(FacialExpression {
            eye_action,
            upper_face_action,
            upper_face_power,
            lower_face_action,
            lower_face_power,
        })
    })))
}

// ─── System Events Stream ───────────────────────────────────────────────

/// Subscribe to the system events stream.
///
/// Returns a stream of [`SysEvent`] containing system-level notifications
/// such as training events and detection results.
///
/// # Errors
/// Returns any error produced by stream channel registration or
/// subscription RPC calls.
pub async fn subscribe_sys(
    client: &CortexClient,
    cortex_token: &str,
    session_id: &str,
) -> CortexResult<Pin<Box<dyn Stream<Item = SysEvent> + Send>>> {
    let rx = add_channel(client, Streams::SYS)?;

    client
        .subscribe_streams(cortex_token, session_id, &[Streams::SYS])
        .await?;

    Ok(Box::pin(TypedStream::new(rx, |event| {
        serde_json::from_value::<SysEvent>(event).ok()
    })))
}

// ─── Unsubscribe ─────────────────────────────────────────────────────────

/// Unsubscribe from one or more data streams and remove the corresponding
/// channels from the client.
///
/// # Errors
/// Returns any error produced by the Cortex `unsubscribe` RPC call.
pub async fn unsubscribe(
    client: &CortexClient,
    cortex_token: &str,
    session_id: &str,
    streams: &[&str],
) -> CortexResult<()> {
    client
        .unsubscribe_streams(cortex_token, session_id, streams)
        .await?;

    for &stream in streams {
        client.remove_stream_channel(stream);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;

    #[tokio::test]
    async fn test_typed_stream_parses_valid_events() {
        let (tx, rx) = mpsc::channel(16);

        let mut stream =
            TypedStream::new(rx, |event| event.get("value")?.as_i64().map(|v| v as i32));

        tx.send(serde_json::json!({"value": 42})).await.unwrap();
        tx.send(serde_json::json!({"value": 99})).await.unwrap();
        drop(tx);

        assert_eq!(stream.next().await, Some(42));
        assert_eq!(stream.next().await, Some(99));
        assert_eq!(stream.next().await, None);
    }

    #[tokio::test]
    async fn test_typed_stream_skips_unparseable_events() {
        let (tx, rx) = mpsc::channel(16);

        let mut stream =
            TypedStream::new(rx, |event| event.get("value")?.as_i64().map(|v| v as i32));

        tx.send(serde_json::json!({"bad": "data"})).await.unwrap();
        tx.send(serde_json::json!({"value": "not_a_number"}))
            .await
            .unwrap();
        tx.send(serde_json::json!({"value": 7})).await.unwrap();
        drop(tx);

        // The first two events should be skipped
        assert_eq!(stream.next().await, Some(7));
        assert_eq!(stream.next().await, None);
    }

    #[tokio::test]
    async fn test_typed_stream_ends_when_sender_dropped() {
        let (tx, rx) = mpsc::channel(16);
        let mut stream = TypedStream::new(rx, |event| event.get("v")?.as_i64().map(|v| v as i32));

        drop(tx);
        assert_eq!(stream.next().await, None);
    }
}
