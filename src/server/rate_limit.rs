#![allow(dead_code)]

use std::time::Instant;

use dashmap::DashMap;

/// Token bucket rate limiter per key
pub struct RateLimiter {
    buckets: DashMap<String, Bucket>,
    max_tokens: u64,
    refill_rate: f64, // tokens per second
}

struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

impl RateLimiter {
    /// Create a new rate limiter.
    /// `max_tokens`: burst capacity
    /// `per_seconds`: refill window (e.g., 60 = max_tokens per minute)
    pub fn new(max_tokens: u64, per_seconds: u64) -> Self {
        Self {
            buckets: DashMap::new(),
            max_tokens,
            refill_rate: max_tokens as f64 / per_seconds as f64,
        }
    }

    /// Try to consume one token for the given key.
    /// Returns (allowed, remaining, reset_seconds)
    pub fn check(&self, key: &str) -> (bool, u64, u64) {
        let mut entry = self.buckets.entry(key.to_owned()).or_insert_with(|| Bucket {
            tokens: self.max_tokens as f64,
            last_refill: Instant::now(),
        });

        let bucket = entry.value_mut();
        let now = Instant::now();
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();

        // Refill tokens
        bucket.tokens = (bucket.tokens + elapsed * self.refill_rate).min(self.max_tokens as f64);
        bucket.last_refill = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            let remaining = bucket.tokens as u64;
            let reset = ((1.0 - bucket.tokens.fract()) / self.refill_rate) as u64;
            (true, remaining, reset)
        } else {
            let reset = ((1.0 - bucket.tokens) / self.refill_rate).ceil() as u64;
            (false, 0, reset)
        }
    }

    pub fn max_tokens(&self) -> u64 {
        self.max_tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allows_within_limit() {
        let limiter = RateLimiter::new(5, 60);
        for _ in 0..5 {
            let (allowed, _, _) = limiter.check("user1");
            assert!(allowed);
        }
    }

    #[test]
    fn test_denies_over_limit() {
        let limiter = RateLimiter::new(2, 60);
        limiter.check("user1");
        limiter.check("user1");
        let (allowed, remaining, _) = limiter.check("user1");
        assert!(!allowed);
        assert_eq!(remaining, 0);
    }

    #[test]
    fn test_separate_keys() {
        let limiter = RateLimiter::new(1, 60);
        let (a1, _, _) = limiter.check("user1");
        let (a2, _, _) = limiter.check("user2");
        assert!(a1);
        assert!(a2);
    }
}
