use time::now_utc;

pub fn get_us() -> u64 {
    let now = now_utc().to_timespec();
    now.sec as u64 * 1000000 + now.nsec as u64 / 1000
}
