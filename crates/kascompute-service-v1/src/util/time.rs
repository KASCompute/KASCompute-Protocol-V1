use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Approx months since start (30-day months; protocol demo)
pub fn months_since(start_unix: u64, now_unix: u64) -> u64 {
    let seconds_per_month = 30 * 24 * 60 * 60;
    now_unix.saturating_sub(start_unix) / seconds_per_month
}
