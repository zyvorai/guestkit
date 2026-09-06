// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Example: Using retry logic

use guestkit::core::retry::{retry_with_backoff, RetryConfig};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config = RetryConfig {
        max_attempts: 5,
        initial_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(30),
        exponential_base: 2.0,
        jitter: true,
    };

    use std::sync::{Arc, Mutex};

    let attempts = Arc::new(Mutex::new(0));
    let attempts_clone = attempts.clone();

    let result = retry_with_backoff(&config, move || {
        let attempts = attempts_clone.clone();
        async move {
            let mut count = attempts.lock().unwrap();
            *count += 1;
            let current = *count;
            drop(count);

            log::info!("Attempt {}", current);

            // Simulate flaky operation
            if current < 3 {
                anyhow::bail!("Temporary failure");
            }

            Ok::<_, anyhow::Error>("Success!")
        }
    })
    .await?;

    println!("✓ {}", result);
    println!("  Total attempts: {}", *attempts.lock().unwrap());

    Ok(())
}
