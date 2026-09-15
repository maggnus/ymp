//! Clocks for wall-clock adjudication of per-invocation limits.
//!
//! The kernel compares caller-supplied elapsed times against the allowance
//! limit; this module supplies those times. Tests install a
//! [`ManualClock`] so timeouts are deterministic; production wiring uses
//! [`SystemClock`].

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A source of elapsed logical time since scenario start.
pub trait Clock: Send + Sync {
    fn elapsed(&self) -> Duration;
}

/// The real clock, anchored at construction.
#[derive(Clone, Copy, Debug)]
pub struct SystemClock {
    started: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
}

/// A deterministic clock advanced only by explicit calls. Clones share one
/// time, so a test can keep a handle while the scenario holds the clock.
#[derive(Clone, Debug, Default)]
pub struct ManualClock {
    elapsed: Arc<Mutex<Duration>>,
}

impl ManualClock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn advance(&self, delta: Duration) {
        if let Ok(mut elapsed) = self.elapsed.lock() {
            *elapsed = elapsed.saturating_add(delta);
        }
    }
}

impl Clock for ManualClock {
    fn elapsed(&self) -> Duration {
        self.elapsed
            .lock()
            .map(|elapsed| *elapsed)
            .unwrap_or_default()
    }
}
