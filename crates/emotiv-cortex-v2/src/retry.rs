//! # Retry Policies
//!
//! Configurable retry logic for Cortex API operations, with exponential
//! backoff and error-category awareness.
//!
//! ## Predefined Policies
//!
//! | Policy | Max Retries | Use Case |
//! |--------|-------------|----------|
//! | [`RetryPolicy::query()`] | 3 | Idempotent reads: `getCortexInfo`, `queryHeadsets`, etc. |
//! | [`RetryPolicy::idempotent()`] | 2 | State-changing but safe to retry: `subscribe`, `controlDevice` |
//! | [`RetryPolicy::none()`] | 0 | Non-idempotent: `authorize`, `createSession`, `injectMarker` |
//!
//! ## Usage
//!
//! ```rust
//! use emotiv_cortex_v2::retry::{RetryPolicy, with_retry};
//! use emotiv_cortex_v2::CortexError;
//! use std::sync::atomic::{AtomicUsize, Ordering};
//!
//! let attempts = AtomicUsize::new(0);
//! let rt = tokio::runtime::Builder::new_current_thread()
//!     .enable_time()
//!     .build()
//!     .unwrap();
//!
//! let result = rt.block_on(async {
//!     with_retry(&RetryPolicy::query(), || {
//!         let attempt = attempts.fetch_add(1, Ordering::SeqCst);
//!         async move {
//!             if attempt == 0 {
//!                 Err(CortexError::Timeout { seconds: 1 })
//!             } else {
//!                 Ok::<_, CortexError>(42)
//!             }
//!         }
//!     })
//!     .await
//! });
//!
//! assert_eq!(result.unwrap(), 42);
//! ```

use std::time::Duration;

use crate::error::{CortexError, CortexResult};

/// Policy controlling how failed operations are retried.
#[derive(Debug, Clone)]
pub enum RetryPolicy {
    /// No retries — fail immediately on error.
    None,

    /// Retry with exponential backoff.
    Backoff {
        /// Maximum number of retry attempts (not counting the initial attempt).
        max_retries: u32,

        /// Initial delay before the first retry.
        base_delay: Duration,

        /// Maximum delay between retries (exponential backoff cap).
        max_delay: Duration,
    },
}

impl RetryPolicy {
    /// No retries. Use for non-idempotent operations like `authorize`,
    /// `createSession`, `createRecord`, `injectMarker`.
    ///
    /// # Examples
    ///
    /// ```
    /// use emotiv_cortex_v2::retry::RetryPolicy;
    ///
    /// let policy = RetryPolicy::none();
    /// assert!(matches!(policy, RetryPolicy::None));
    /// ```
    #[must_use]
    pub fn none() -> Self {
        Self::None
    }

    /// 3 retries with 500ms base delay. Use for idempotent query operations
    /// like `getCortexInfo`, `queryHeadsets`, `querySessions`, `queryRecords`.
    #[must_use]
    pub fn query() -> Self {
        Self::Backoff {
            max_retries: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
        }
    }

    /// 2 retries with 1s base delay. Use for idempotent state-changing
    /// operations like `controlDevice`, `subscribe`, `unsubscribe`,
    /// `setupProfile(load)`.
    #[must_use]
    pub fn idempotent() -> Self {
        Self::Backoff {
            max_retries: 2,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(15),
        }
    }

    /// 2 retries with 1s base delay. Use for idempotent stopping operations
    /// like `updateSession(close)`, `updateRecord(stop)`.
    #[must_use]
    pub fn stop() -> Self {
        Self::Backoff {
            max_retries: 2,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(15),
        }
    }

    /// Custom backoff policy.
    ///
    /// # Examples
    ///
    /// ```
    /// use emotiv_cortex_v2::retry::RetryPolicy;
    /// use std::time::Duration;
    ///
    /// let policy = RetryPolicy::custom(5, Duration::from_millis(200), Duration::from_secs(30));
    /// ```
    #[must_use]
    pub fn custom(max_retries: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self::Backoff {
            max_retries,
            base_delay,
            max_delay,
        }
    }
}

/// Execute an async operation with retry logic.
///
/// The operation is retried according to the policy when the error is
/// retryable (as determined by [`CortexError::is_retryable()`]).
/// Non-retryable errors are returned immediately regardless of the policy.
///
/// On exhaustion, returns [`CortexError::RetriesExhausted`] wrapping
/// the last error encountered.
///
/// # Errors
/// Returns any error from the operation, including a wrapped
/// [`CortexError::RetriesExhausted`] when retry attempts are exhausted.
pub async fn with_retry<F, Fut, T>(policy: &RetryPolicy, mut operation: F) -> CortexResult<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = CortexResult<T>>,
{
    match policy {
        RetryPolicy::None => operation().await,
        RetryPolicy::Backoff {
            max_retries,
            base_delay,
            max_delay,
        } => {
            let mut delay = *base_delay;

            for attempt in 0..=*max_retries {
                match operation().await {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        // Non-retryable errors fail immediately
                        if !e.is_retryable() {
                            return Err(e);
                        }

                        // Last attempt — wrap in RetriesExhausted
                        if attempt == *max_retries {
                            return Err(CortexError::RetriesExhausted {
                                attempts: attempt + 1,
                                last_error: Box::new(e),
                            });
                        }

                        tracing::warn!(
                            attempt = attempt + 1,
                            max = max_retries + 1,
                            error = %e,
                            delay_ms = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX),
                            "Retrying after transient error"
                        );

                        tokio::time::sleep(delay).await;

                        // Exponential backoff with cap
                        delay = std::cmp::min(delay * 2, *max_delay);
                    }
                }
            }

            // Should be unreachable, but handle gracefully
            operation().await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_no_retry_succeeds() {
        let result = with_retry(&RetryPolicy::none(), || async { Ok::<_, CortexError>(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_no_retry_fails_immediately() {
        let result = with_retry(&RetryPolicy::none(), || async {
            Err::<i32, _>(CortexError::Timeout { seconds: 5 })
        })
        .await;
        assert!(matches!(result.unwrap_err(), CortexError::Timeout { .. }));
    }

    #[tokio::test]
    async fn test_retry_succeeds_after_transient_failure() {
        let attempts = AtomicU32::new(0);

        let result = with_retry(
            &RetryPolicy::custom(3, Duration::from_millis(1), Duration::from_millis(10)),
            || {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt < 2 {
                        Err(CortexError::Timeout { seconds: 1 })
                    } else {
                        Ok(42)
                    }
                }
            },
        )
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts.load(Ordering::SeqCst), 3); // 2 failures + 1 success
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let attempts = AtomicU32::new(0);

        let result = with_retry(
            &RetryPolicy::custom(2, Duration::from_millis(1), Duration::from_millis(10)),
            || {
                attempts.fetch_add(1, Ordering::SeqCst);
                async { Err::<i32, _>(CortexError::Timeout { seconds: 1 }) }
            },
        )
        .await;

        match result.unwrap_err() {
            CortexError::RetriesExhausted { attempts: a, .. } => assert_eq!(a, 3),
            other => panic!("Expected RetriesExhausted, got {other:?}"),
        }
        assert_eq!(attempts.load(Ordering::SeqCst), 3); // initial + 2 retries
    }

    #[tokio::test]
    async fn test_retry_exhausted_preserves_last_error() {
        let result = with_retry(
            &RetryPolicy::custom(1, Duration::from_millis(1), Duration::from_millis(10)),
            || async { Err::<i32, _>(CortexError::Timeout { seconds: 5 }) },
        )
        .await;

        let err = result.unwrap_err();
        let CortexError::RetriesExhausted {
            attempts,
            last_error,
        } = &err
        else {
            panic!("expected RetriesExhausted, got {err:?}");
        };
        assert_eq!(*attempts, 2); // initial + 1 retry
        assert!(matches!(
            last_error.as_ref(),
            CortexError::Timeout { seconds: 5 }
        ));
        assert!(last_error.is_retryable());
    }

    #[tokio::test]
    async fn test_non_retryable_error_not_retried() {
        let attempts = AtomicU32::new(0);

        let result = with_retry(
            &RetryPolicy::custom(3, Duration::from_millis(1), Duration::from_millis(10)),
            || {
                attempts.fetch_add(1, Ordering::SeqCst);
                async { Err::<i32, _>(CortexError::NoHeadsetFound) }
            },
        )
        .await;

        assert!(matches!(result.unwrap_err(), CortexError::NoHeadsetFound));
        assert_eq!(attempts.load(Ordering::SeqCst), 1); // Only tried once
    }

    #[tokio::test]
    async fn test_backoff_policy_succeeds_on_first_try() {
        let result = with_retry(&RetryPolicy::query(), || async { Ok::<_, CortexError>(99) }).await;
        assert_eq!(result.unwrap(), 99);
    }

    #[test]
    fn test_policy_constructor_defaults() {
        match RetryPolicy::query() {
            RetryPolicy::Backoff {
                max_retries,
                base_delay,
                max_delay,
            } => {
                assert_eq!(max_retries, 3);
                assert_eq!(base_delay, Duration::from_millis(500));
                assert_eq!(max_delay, Duration::from_secs(10));
            }
            RetryPolicy::None => panic!("query policy should use backoff"),
        }

        match RetryPolicy::idempotent() {
            RetryPolicy::Backoff {
                max_retries,
                base_delay,
                max_delay,
            } => {
                assert_eq!(max_retries, 2);
                assert_eq!(base_delay, Duration::from_secs(1));
                assert_eq!(max_delay, Duration::from_secs(15));
            }
            RetryPolicy::None => panic!("idempotent policy should use backoff"),
        }

        match RetryPolicy::stop() {
            RetryPolicy::Backoff {
                max_retries,
                base_delay,
                max_delay,
            } => {
                assert_eq!(max_retries, 2);
                assert_eq!(base_delay, Duration::from_secs(1));
                assert_eq!(max_delay, Duration::from_secs(15));
            }
            RetryPolicy::None => panic!("stop policy should use backoff"),
        }
    }

    #[tokio::test]
    async fn test_backoff_delay_caps_at_max_delay() {
        let attempts = AtomicU32::new(0);
        let start = std::time::Instant::now();

        let result = with_retry(
            &RetryPolicy::custom(3, Duration::from_millis(1), Duration::from_millis(2)),
            || {
                attempts.fetch_add(1, Ordering::SeqCst);
                async { Err::<(), _>(CortexError::Timeout { seconds: 1 }) }
            },
        )
        .await;

        assert!(matches!(
            result.unwrap_err(),
            CortexError::RetriesExhausted { .. }
        ));
        assert_eq!(attempts.load(Ordering::SeqCst), 4); // initial + 3 retries
        assert!(
            start.elapsed() >= Duration::from_millis(4),
            "elapsed {:?} was too short for capped backoff",
            start.elapsed()
        );
    }
}
