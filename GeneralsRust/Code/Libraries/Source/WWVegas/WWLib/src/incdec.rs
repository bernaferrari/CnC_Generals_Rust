// Auto-generated C++ compatibility shim for increment/decrement helpers
pub fn inc_clamp(value: i32, max: i32) -> i32 {
    if value >= max {
        max
    } else {
        value + 1
    }
}

pub fn dec_clamp(value: i32, min: i32) -> i32 {
    if value <= min {
        min
    } else {
        value - 1
    }
}
