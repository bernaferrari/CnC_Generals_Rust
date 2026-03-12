// Auto-generated C++ compatibility shim for multimedia timing
use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();

pub fn time_get_time() -> u32 {
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_millis() as u32
}
