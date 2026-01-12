//! Google SRE（eq2101）判定算法（纯计算，不绑定窗口实现）。
//!
//! - 统计采集：一般来自滚动/固定窗口或计数器（见 `crate::collection::metrics`）
//! - 本文件只负责：给定窗口统计快照，计算 drop_ratio，并决定是否拒绝
//!
//! 参考：
//! - SRE eq2101：`https://landing.google.com/sre/sre-book/chapters/handling-overload/#eq2101`

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// 统计快照（Google SRE eq2101 的输入）。
#[derive(Debug, Default, Clone, Copy)]
pub struct Snapshot {
    pub accepts: i64,
    pub total: i64,
    pub failing_buckets: i64,
    pub working_buckets: i64,
    pub buckets: usize,
}

const DEFAULT_FORCE_PASS: Duration = Duration::from_secs(1);
const DEFAULT_K: f64 = 1.5;
const DEFAULT_MIN_K: f64 = 1.1;
const DEFAULT_PROTECTION: i64 = 5;

fn now_ns() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_nanos() as u64
}

/// Google SRE(eq2101) 参数。
#[derive(Debug, Clone, Copy)]
pub struct GoogleSreParams {
    pub force_pass: Duration,
    pub k: f64,
    pub min_k: f64,
    pub protection: i64,
}

impl Default for GoogleSreParams {
    fn default() -> Self {
        Self {
            force_pass: DEFAULT_FORCE_PASS,
            k: DEFAULT_K,
            min_k: DEFAULT_MIN_K,
            protection: DEFAULT_PROTECTION,
        }
    }
}

/// 计算 drop_ratio（SRE eq2101 的核心公式）。
pub fn compute_drop_ratio(history: Snapshot, params: GoogleSreParams) -> f64 {
    let buckets_f = history.buckets.max(1) as f64;
    let failing = history.failing_buckets as f64;
    let working = history.working_buckets as f64;

    // w = k - (k-minK)*failingBuckets/buckets
    let w = params.k - (params.k - params.min_k) * failing / buckets_f;
    let w = w.max(params.min_k);
    let weighted_accepts = w * history.accepts as f64;

    // eq2101：dropRatio = (total-protection - weightedAccepts)/(total+1)
    let total_f = history.total as f64;
    let drop_ratio = (total_f - params.protection as f64 - weighted_accepts) / (total_f + 1.0);
    if drop_ratio <= 0.0 {
        return drop_ratio;
    }

    // dropRatio *= (buckets-workingBuckets)/buckets
    drop_ratio * (buckets_f - working) / buckets_f
}

/// 判定器：基于 drop_ratio 做“概率拒绝”。
#[derive(Debug)]
pub struct GoogleSreJudge {
    params: GoogleSreParams,
    last_pass_ns: AtomicU64,
    seq: AtomicU64,
}

impl GoogleSreJudge {
    pub fn new(params: GoogleSreParams) -> Self {
        Self {
            params,
            last_pass_ns: AtomicU64::new(0),
            seq: AtomicU64::new(0),
        }
    }

    fn should_drop(&self, ratio: f64) -> bool {
        if ratio <= 0.0 {
            return false;
        }
        let r = ratio.clamp(0.0, 1.0);
        // 确定性采样（避免引入 RNG）：seq % 1_000_000 < r * 1_000_000
        let threshold = (r * 1_000_000.0).round() as u64;
        let s = self.seq.fetch_add(1, Ordering::Relaxed);
        (s % 1_000_000) < threshold
    }

    /// 返回 true 表示应拒绝（open/drop）。
    pub fn should_reject(&self, _now: Instant, snapshot: Snapshot) -> bool {
        let drop_ratio = compute_drop_ratio(snapshot, self.params);
        let now_ns = now_ns();

        if drop_ratio <= 0.0 {
            self.last_pass_ns.store(now_ns, Ordering::Relaxed);
            return false;
        }

        // 强制放行：超过 force_pass 间隔则放行一次
        let last = self.last_pass_ns.load(Ordering::Relaxed);
        if last > 0 && now_ns.saturating_sub(last) > self.params.force_pass.as_nanos() as u64 {
            self.last_pass_ns.store(now_ns, Ordering::Relaxed);
            return false;
        }

        if self.should_drop(drop_ratio) {
            return true;
        }

        self.last_pass_ns.store(now_ns, Ordering::Relaxed);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_ratio_positive_when_total_large_accepts_small() {
        let s = Snapshot {
            accepts: 0,
            total: 100,
            failing_buckets: 0,
            working_buckets: 0,
            buckets: 40,
        };
        let r = compute_drop_ratio(s, GoogleSreParams::default());
        assert!(r > 0.0);
    }
}
