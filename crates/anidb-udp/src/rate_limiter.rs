//! Flood protection: a minimum time between the starts of two requests. The
//! spec's flood protection rule is described with [`crate::constants`], see the
//! crate root for the link.

use std::thread;
use std::time::{Duration, Instant};

/// Keeps a minimum interval between the starts of two requests.
///
/// The client is blocking with one request in flight, so there is no queue:
/// [`RateLimiter::wait`] sleeps until the next request may start and records
/// that start. The first request never waits.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    interval: Duration,
    last_start: Option<Instant>,
}

impl RateLimiter {
    /// A limiter that spaces request starts by `interval`.
    #[must_use]
    pub const fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_start: None,
        }
    }

    /// The time left before the next request may start, zero when it may start now.
    #[must_use]
    pub fn remaining(&self) -> Duration {
        self.last_start.map_or(Duration::ZERO, |last| {
            (last + self.interval).saturating_duration_since(Instant::now())
        })
    }

    /// Sleeps until a request may start, then records the start. Returns how
    /// long it slept.
    pub fn wait(&mut self) -> Duration {
        let wait = self.remaining();
        if !wait.is_zero() {
            thread::sleep(wait);
        }
        self.last_start = Some(Instant::now());
        wait
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_request_does_not_wait() {
        let mut limiter = RateLimiter::new(Duration::from_secs(5));
        assert_eq!(limiter.remaining(), Duration::ZERO);
        assert_eq!(limiter.wait(), Duration::ZERO);
    }

    #[test]
    fn starts_are_spaced_by_the_interval() {
        let interval = Duration::from_millis(50);
        let mut limiter = RateLimiter::new(interval);
        let mut starts = Vec::new();
        for _ in 0..4 {
            limiter.wait();
            starts.push(Instant::now());
        }
        for pair in starts.windows(2) {
            assert!(pair[1] - pair[0] >= interval, "{:?}", pair[1] - pair[0]);
        }
    }

    #[test]
    fn time_spent_working_counts_towards_the_interval() {
        let mut limiter = RateLimiter::new(Duration::from_millis(60));
        limiter.wait();
        thread::sleep(Duration::from_millis(80));
        assert_eq!(limiter.remaining(), Duration::ZERO);
    }
}
