// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Retry logic with exponential backoff

use rand::Rng;
use std::time::Duration;
use tokio::time::sleep;

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub exponential_base: f64,
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            exponential_base: 2.0,
            jitter: true,
        }
    }
}

/// Retry a function with exponential backoff
///
/// # Examples
///
/// ```no_run
/// use guestkit::core::retry::{retry_with_backoff, RetryConfig};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let config = RetryConfig::default();
///     let result = retry_with_backoff(&config, || async {
///         // Flaky operation
///         Ok::<_, anyhow::Error>("Success!")
///     }).await?;
///
///     println!("{}", result);
///     Ok(())
/// }
/// ```
pub async fn retry_with_backoff<F, Fut, T, E>(
    config: &RetryConfig,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    assert!(
        config.max_attempts > 0,
        "retry_with_backoff called with max_attempts == 0"
    );

    let mut last_err = None;
    for attempt in 1..=config.max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt == config.max_attempts {
                    log::error!("Operation failed after {} attempts: {}", attempt, e);
                    last_err = Some(e);
                    break;
                }

                // Calculate delay with exponential backoff
                let delay_secs = (config.initial_delay.as_secs_f64()
                    * config.exponential_base.powi((attempt - 1) as i32))
                .min(config.max_delay.as_secs_f64());

                let mut delay = Duration::from_secs_f64(delay_secs);

                // Add jitter to prevent thundering herd
                if config.jitter {
                    let jitter_factor = 0.5 + rand::thread_rng().r#gen::<f64>() * 0.5;
                    delay = Duration::from_secs_f64(delay.as_secs_f64() * jitter_factor);
                }

                log::warn!(
                    "Operation failed (attempt {}/{}): {}. Retrying in {:.2}s...",
                    attempt,
                    config.max_attempts,
                    e,
                    delay.as_secs_f64()
                );

                sleep(delay).await;
            }
        }
    }

    // last_err is always Some here: the loop runs at least once (max_attempts >= 1),
    // and exit via break always sets last_err.
    Err(last_err.expect("retry loop ran but produced no error"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn test_retry_success_on_first_attempt() {
        let config = RetryConfig::default();
        let attempts = Arc::new(Mutex::new(0));
        let attempts_clone = attempts.clone();

        let result = retry_with_backoff(&config, move || {
            let attempts = attempts_clone.clone();
            async move {
                *attempts.lock().unwrap() += 1;
                Ok::<_, anyhow::Error>(42)
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(*attempts.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let config = RetryConfig {
            max_attempts: 5,
            initial_delay: Duration::from_millis(10),
            ..Default::default()
        };
        let attempts = Arc::new(Mutex::new(0));
        let attempts_clone = attempts.clone();

        let result = retry_with_backoff(&config, move || {
            let attempts = attempts_clone.clone();
            async move {
                let mut count = attempts.lock().unwrap();
                *count += 1;
                let current = *count;
                drop(count);

                if current < 3 {
                    anyhow::bail!("Temporary failure");
                }
                Ok::<_, anyhow::Error>(42)
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(*attempts.lock().unwrap(), 3);
    }

    #[tokio::test]
    async fn test_retry_exhausts_attempts() {
        let config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(10),
            ..Default::default()
        };
        let attempts = Arc::new(Mutex::new(0));
        let attempts_clone = attempts.clone();

        let result: Result<i32, anyhow::Error> = retry_with_backoff(&config, move || {
            let attempts = attempts_clone.clone();
            async move {
                *attempts.lock().unwrap() += 1;
                anyhow::bail!("Persistent failure")
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(*attempts.lock().unwrap(), 3);
    }
}
