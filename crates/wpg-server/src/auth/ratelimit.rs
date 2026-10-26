//! Login rate limit: a token bucket per client address with exponential
//! lock-out (spec §2.6 "ログイン失敗レート制限").
//!
//! Every failed login takes a token from the client's bucket ([`CAPACITY`]
//! tokens, refilled at [`CAPACITY`] per minute). When the bucket runs dry the
//! client is locked out for [`BASE_LOCK`], doubling with every further lock-out
//! up to [`MAX_LOCK`]. A successful login, or [`MAX_LOCK`] without failures,
//! forgets the client. State is in memory; every attempt is also written to the
//! `login_attempts` table for auditing.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Failed attempts allowed per minute.
pub const CAPACITY: f64 = 5.0;
/// First lock-out.
pub const BASE_LOCK: Duration = Duration::from_secs(60);
/// Longest lock-out.
pub const MAX_LOCK: Duration = Duration::from_secs(15 * 60);
/// Entries kept before idle ones are dropped.
const MAX_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, Copy)]
struct Entry {
    tokens: f64,
    updated: Instant,
    locked_until: Option<Instant>,
    strikes: u32,
}

/// Per-key login limiter.
#[derive(Debug, Default)]
pub struct RateLimiter {
    map: Mutex<HashMap<String, Entry>>,
}

impl RateLimiter {
    /// An empty limiter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `Err(wait)` while `key` is locked out.
    ///
    /// # Errors
    /// The remaining lock-out.
    pub fn check(&self, key: &str, now: Instant) -> Result<(), Duration> {
        let map = self.map.lock().unwrap_or_else(PoisonError::into_inner);
        match map.get(key).and_then(|e| e.locked_until) {
            Some(until) if until > now => Err(until - now),
            _ => Ok(()),
        }
    }

    /// Records a failure; returns the lock-out it started, if any.
    pub fn failure(&self, key: &str, now: Instant) -> Option<Duration> {
        let mut map = self.map.lock().unwrap_or_else(PoisonError::into_inner);
        if map.len() >= MAX_ENTRIES {
            map.retain(|_, e| now.saturating_duration_since(e.updated) < MAX_LOCK);
        }
        let e = map.entry(key.to_owned()).or_insert(Entry {
            tokens: CAPACITY,
            updated: now,
            locked_until: None,
            strikes: 0,
        });
        let idle = now.saturating_duration_since(e.updated);
        let quiet_since = e.locked_until.map_or(e.updated, |u| u.max(e.updated));
        if now.saturating_duration_since(quiet_since) >= MAX_LOCK {
            e.strikes = 0;
        }
        e.tokens = (e.tokens + idle.as_secs_f64() * CAPACITY / 60.0).min(CAPACITY) - 1.0;
        e.updated = now;
        if e.tokens < 1.0 {
            e.strikes += 1;
            let lock = BASE_LOCK
                .saturating_mul(1 << (e.strikes - 1).min(10))
                .min(MAX_LOCK);
            e.locked_until = Some(now + lock);
            e.tokens = 1.0;
            Some(lock)
        } else {
            None
        }
    }

    /// Forgets `key` after a successful login.
    pub fn success(&self, key: &str) {
        self.map
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_per_minute_then_exponential_lockout() {
        let rl = RateLimiter::new();
        let t0 = Instant::now();
        for i in 0..4 {
            assert!(rl.check("a", t0).is_ok());
            assert_eq!(rl.failure("a", t0), None, "attempt {i}");
        }
        assert_eq!(rl.failure("a", t0), Some(BASE_LOCK));
        assert_eq!(rl.check("a", t0), Err(BASE_LOCK));
        // other clients are unaffected
        assert!(rl.check("b", t0).is_ok());

        // after the lock-out one more failure locks for twice as long
        let t1 = t0 + BASE_LOCK;
        assert!(rl.check("a", t1).is_ok());
        let mut lock = None;
        let mut n = 0;
        while lock.is_none() {
            lock = rl.failure("a", t1);
            n += 1;
        }
        assert!(n <= 5, "{n}");
        assert_eq!(lock, Some(BASE_LOCK * 2));
        // capped
        let mut t = t1;
        let mut lock = BASE_LOCK * 2;
        for _ in 0..6 {
            t += lock;
            lock = loop {
                if let Some(l) = rl.failure("a", t) {
                    break l;
                }
            };
        }
        assert_eq!(lock, MAX_LOCK);
        assert_eq!(rl.check("a", t), Err(MAX_LOCK));
        // forgotten after a quiet period
        t += MAX_LOCK * 2;
        assert!(rl.check("a", t).is_ok());
        assert_eq!(rl.failure("a", t), None);

        // success forgets
        rl.success("a");
        assert!(rl.check("a", t).is_ok());
    }

    #[test]
    fn slow_failures_never_lock() {
        let rl = RateLimiter::new();
        let mut t = Instant::now();
        for _ in 0..50 {
            assert_eq!(rl.failure("a", t), None);
            t += Duration::from_secs(13);
        }
    }
}
