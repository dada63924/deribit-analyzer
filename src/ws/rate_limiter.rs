use std::collections::HashMap;
use std::time::Instant;
use tokio::time::{sleep, Duration};
use tracing::debug;

/// Credit pool for a specific endpoint category
struct CreditPool {
    credits: f64,
    max_credits: f64,
    refill_rate: f64, // credits per second
    last_update: Instant,
}

impl CreditPool {
    fn new(max_credits: f64, refill_rate: f64) -> Self {
        CreditPool {
            credits: max_credits,
            max_credits,
            refill_rate,
            last_update: Instant::now(),
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update).as_secs_f64();
        self.credits = (self.credits + elapsed * self.refill_rate).min(self.max_credits);
        self.last_update = now;
    }

    fn try_consume(&mut self, cost: f64) -> Option<Duration> {
        self.refill();
        if self.credits >= cost {
            self.credits -= cost;
            None
        } else {
            let deficit = cost - self.credits;
            let wait_secs = deficit / self.refill_rate;
            Some(Duration::from_secs_f64(wait_secs))
        }
    }
}

/// Rate limit categories with their credit costs
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum RateLimitCategory {
    /// Non-matching engine requests: 500 credits, pool 50,000, refill 10,000/s
    NonMatching,
    /// Subscribe requests: 3,000 credits, pool 30,000, refill 10,000/s
    Subscribe,
    /// Get instruments: 10,000 credits, pool 500,000, refill 10,000/s
    GetInstruments,
}

impl RateLimitCategory {
    fn cost(&self) -> f64 {
        match self {
            RateLimitCategory::NonMatching => 500.0,
            RateLimitCategory::Subscribe => 3000.0,
            RateLimitCategory::GetInstruments => 10000.0,
        }
    }
}

pub struct RateLimiter {
    pools: HashMap<RateLimitCategory, CreditPool>,
}

impl RateLimiter {
    pub fn new() -> Self {
        let mut pools = HashMap::new();
        pools.insert(
            RateLimitCategory::NonMatching,
            CreditPool::new(50_000.0, 10_000.0),
        );
        pools.insert(
            RateLimitCategory::Subscribe,
            CreditPool::new(30_000.0, 10_000.0),
        );
        pools.insert(
            RateLimitCategory::GetInstruments,
            CreditPool::new(500_000.0, 10_000.0),
        );
        RateLimiter { pools }
    }

    /// Categorize a method into its rate limit category
    pub fn categorize(method: &str) -> RateLimitCategory {
        match method {
            "public/subscribe" | "private/subscribe" => RateLimitCategory::Subscribe,
            "public/get_instruments" => RateLimitCategory::GetInstruments,
            _ => RateLimitCategory::NonMatching,
        }
    }

    /// Wait until credits are available, then consume them
    pub async fn acquire(&mut self, method: &str) {
        let category = Self::categorize(method);
        let cost = category.cost();

        let pool = self.pools.get_mut(&category).expect("unknown category");
        if let Some(wait) = pool.try_consume(cost) {
            debug!(
                method = method,
                wait_ms = wait.as_millis() as u64,
                "Rate limit: waiting for credits"
            );
            sleep(wait).await;
            // After sleeping, refill and consume
            pool.refill();
            pool.credits -= cost;
        }
    }
}
