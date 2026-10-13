//! Optional immutable Main observation configuration.
mod env;
pub use env::*;

pub fn gameworld_shadow_enabled() -> bool {
    env_flag_raw("GENERALS_GAMEWORLD_SHADOW", true)
}
