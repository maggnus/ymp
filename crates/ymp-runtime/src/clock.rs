//! Explicit milliseconds supplied to kernel commands; replay never reads the host clock.
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use ymp_domain::{Denial, Result};
pub trait Clock: Send + Sync {
    fn now(&self) -> Result<u64>;
}
pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> Result<u64> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Denial::new("clock", "System time predates the Unix epoch"))?;
        u64::try_from(elapsed.as_millis())
            .map_err(|_| Denial::new("clock", "System milliseconds exceed the supported range"))
    }
}
pub struct ManualClock(AtomicU64);
impl ManualClock {
    pub fn new(at: u64) -> Self {
        Self(AtomicU64::new(at))
    }
    pub fn advance_to(&self, at: u64) -> Result<()> {
        self.0
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |prior| {
                (at >= prior).then_some(at)
            })
            .map(|_| ())
            .map_err(|_| Denial::new("clock", "A controllable clock cannot move backwards"))
    }
}
impl Clock for ManualClock {
    fn now(&self) -> Result<u64> {
        Ok(self.0.load(Ordering::SeqCst))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clocks_use_epoch_milliseconds_and_manual_time_does_not_regress() {
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let observed = u128::from(SystemClock.now().unwrap());
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        assert!((before..=after).contains(&observed));
        let manual = ManualClock::new(10);
        assert!(manual.advance_to(9).is_err());
        assert_eq!(manual.now().unwrap(), 10);
        manual.advance_to(11).unwrap();
        assert_eq!(manual.now().unwrap(), 11);
    }
}
