//! Game-logic and client random helpers
//!
//! Split from `helpers.rs` for module-size parity.
//! Observable behavior is unchanged.

use super::*;

/// Gets game logic random integer value (matching C++ GetGameLogicRandomValue).
///
/// Unified with `game_engine::common::random_value` Common stream residual.
pub fn get_game_logic_random_value(lo: Int, hi: Int) -> Int {
    if hi < lo {
        return hi;
    }
    game_engine::common::random_value::get_game_logic_random_value(lo, hi)
}

/// Gets game logic random u32 value (convenience wrapper for u32 params)
/// Matches C++ GetGameLogicRandomValue behavior
pub fn game_logic_random_value(lo: u32, hi: u32) -> u32 {
    if hi < lo {
        return hi;
    }
    game_engine::common::random_value::get_game_logic_random_value(lo as i32, hi as i32) as u32
}

/// Gets game logic random real value (matching C++ GetGameLogicRandomValueReal)
pub fn get_game_logic_random_value_real(lo: Real, hi: Real) -> Real {
    if hi <= lo {
        return hi;
    }
    game_engine::common::random_value::get_game_logic_random_value_real(lo, hi)
}

/// Client-side random value (visual-only; not network-synchronized).
/// Unified with Common client stream residual.
pub fn game_client_random_value(lo: Int, hi: Int) -> Int {
    if hi < lo {
        return hi;
    }
    game_engine::common::random_value::get_game_client_random_value(lo, hi)
}

/// Client-side random real (visual-only; not network-synchronized).
pub fn game_client_random_value_real(lo: Real, hi: Real) -> Real {
    if hi <= lo {
        return hi;
    }
    game_engine::common::random_value::get_game_client_random_value_real(lo, hi)
}

/// Gets the CRC of the game logic random seed state (matching C++ GetGameLogicRandomSeedCRC)
/// CRITICAL for network synchronization - ensures all players have same random state.
///
/// Residual: hashes the Common stream 6-word seed via Generals rotate-add CRC.
pub fn get_game_logic_random_seed_crc() -> UnsignedInt {
    game_engine::common::random_value::get_game_logic_random_seed_crc()
}

/// Sets the game logic random seed (matching C++ SetGameLogicRandomSeed).
///
/// Writes into the Common stream residual so helpers and Main host RNG share one ADC state.
pub fn set_game_logic_random_seed(new_seed: [u32; 6]) {
    game_engine::common::random_value::set_game_logic_random_seed_state(new_seed);
}

/// Game logic random value macro (matching C++ GameLogicRandomValue macro)
#[macro_export]
macro_rules! GameLogicRandomValue {
    ($lo:expr, $hi:expr) => {
        $crate::helpers::get_game_logic_random_value($lo as i32, $hi as i32)
    };
}

/// Game logic random real value macro (matching C++ GameLogicRandomValueReal macro)
#[macro_export]
macro_rules! GameLogicRandomValueReal {
    ($lo:expr, $hi:expr) => {
        $crate::helpers::get_game_logic_random_value_real($lo, $hi)
    };
}

/// Client random real value macro (matching C++ GameClientRandomValueReal macro)
#[macro_export]
macro_rules! GameClientRandomValueReal {
    ($lo:expr, $hi:expr) => {
        $crate::helpers::game_client_random_value_real($lo, $hi)
    };
}

/// Make object status mask macro (matching C++ MAKE_OBJECT_STATUS_MASK).
#[macro_export]
macro_rules! MAKE_OBJECT_STATUS_MASK {
    ($status:expr) => {
        $crate::common::ObjectStatusMaskType::from_status($status)
    };
}

/// Make model condition mask macro (matching C++ MAKE_MODELCONDITION_MASK).
#[macro_export]
macro_rules! MAKE_MODELCONDITION_MASK {
    ($condition:expr) => {
        $condition
    };
}
