use crate::middleware::{Middleware, middleware};
use hyper::Body;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::Instant;

/// Simple token bucket with async wait; burst allowed.
pub fn rate_limit(permits_per_sec: u64, burst: u64) -> Middleware {
    let bucket = Arc::new(TokenBucket::new(permits_per_sec, burst));
    middleware(move |req, next| {
        let bucket = bucket.clone();
        async move {
            if bucket.acquire().await {
                next.call(req).await
            } else {
                http::Response::builder()
                    .status(http::StatusCode::TOO_MANY_REQUESTS)
                    .body(Body::from("rate limited"))
                    .unwrap()
            }
        }
    })
}

/// Concurrency limit with immediate reject.
pub fn concurrency_limit(limit: usize) -> Middleware {
    let sem = Arc::new(Semaphore::new(limit));
    middleware(move |req, next| {
        let sem = sem.clone();
        async move {
            let permit = sem.try_acquire_owned();
            if let Ok(permit) = permit {
                let resp = next.call(req).await;
                drop(permit);
                resp
            } else {
                http::Response::builder()
                    .status(http::StatusCode::SERVICE_UNAVAILABLE)
                    .body(Body::from("too many in-flight requests"))
                    .unwrap()
            }
        }
    })
}

struct TokenBucket {
    permits_per_sec: u64,
    burst: u64,
    available: tokio::sync::Mutex<BucketState>,
}

struct BucketState {
    tokens: u64,
    last: Instant,
}

impl TokenBucket {
    fn new(permits_per_sec: u64, burst: u64) -> Self {
        Self {
            permits_per_sec,
            burst,
            available: tokio::sync::Mutex::new(BucketState {
                tokens: burst,
                last: Instant::now(),
            }),
        }
    }

    async fn acquire(&self) -> bool {
        let mut state = self.available.lock().await;
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(state.last).as_secs_f64();
        if elapsed > 0.0 {
            let add = (elapsed * self.permits_per_sec as f64).floor() as u64;
            state.tokens = (state.tokens + add).min(self.burst);
            state.last = now;
        }
        if state.tokens > 0 {
            state.tokens -= 1;
            true
        } else {
            false
        }
    }
}
