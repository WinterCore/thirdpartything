use std::time::{self, UNIX_EPOCH};

pub fn now_secs() -> u64 {
    time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}
