use std::time::SystemTime;

pub fn get_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
