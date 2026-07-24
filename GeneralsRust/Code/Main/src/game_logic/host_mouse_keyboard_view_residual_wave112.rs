//! Wave 112 residual peels: Mouse / Keyboard / View input residual
//! (host-testable client input path; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 79 input residual, Wave 86 camera/FPS GameData, Wave 88
//! mouse-cursor name table, Wave 110 MessageStream/MetaEvent.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - Mouse.h MouseButtonState, MouseCursor / NUM_MOUSE_CURSORS, RedrawMode,
//!   click sensitivity, wheel delta, event queue size
//! - Keyboard.h NUM_KEYS / KEY_NAMES_COUNT / MAX_KEY_STATES / KEY_REPEAT_DELAY
//! - View.h DEFAULT_VIEW_*, CameraShakeType, WorldToScreenReturn, PickType bits
//!
//! Fail-closed:
//! - Not full Mouse INI cursor load / hardware DX8 cursor residual
//! - Not full Keyboard translateKey locale table residual
//! - Not full View 3D camera / filter mode residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Mouse residual
// ---------------------------------------------------------------------------

/// Retail `MouseButtonState`.
pub const MOUSE_BUTTON_STATE_NAMES: &[&str] = &["Up", "Down", "DoubleClick"];
pub const MOUSE_BUTTON_STATE_COUNT: usize = 3;

/// Retail mouse move modes.
pub const MOUSE_MOVE_RELATIVE: u32 = 0;
pub const MOUSE_MOVE_ABSOLUTE: u32 = 1;

/// Retail click residual constants.
pub const CLICK_SENSITIVITY: u32 = 15;
pub const CLICK_DISTANCE_DELTA: u32 = 10;
pub const CLICK_DISTANCE_DELTA_SQUARED: u32 = CLICK_DISTANCE_DELTA * CLICK_DISTANCE_DELTA;
pub const MOUSE_WHEEL_DELTA: u32 = 120;

/// Retail mouse status residual.
pub const MOUSE_NONE: u32 = 0x00;
pub const MOUSE_OK: u32 = 0x01;
pub const MOUSE_FAILED: u32 = 0x80;
pub const MOUSE_LOST: u32 = 0xFF;
pub const MOUSE_EVENT_NONE: u32 = 0x00;

/// Retail 2D cursor residual.
pub const MAX_2D_CURSOR_ANIM_FRAMES: u32 = 21;
pub const MAX_2D_CURSOR_DIRECTIONS: u32 = 8;

/// Retail mouse event queue residual.
pub const NUM_MOUSE_EVENTS: u32 = 256;

/// Retail `MouseCursor` residual names (matches `CursorININames`, no ALLOW_DEMORALIZE/SURRENDER).
pub const MOUSE_CURSOR_INI_NAMES: &[&str] = &[
    "None",
    "Normal",
    "Arrow",
    "Scroll",
    "Target",
    "Move",
    "AttackMove",
    "AttackObj",
    "ForceAttackObj",
    "ForceAttackGround",
    "Build",
    "InvalidBuild",
    "GenericInvalid",
    "Select",
    "EnterFriendly",
    "EnterAggressive",
    "SetRallyPoint",
    "GetRepaired",
    "GetHealed",
    "DoRepair",
    "ResumeConstruction",
    "CaptureBuilding",
    "SnipeVehicle",
    "LaserGuidedMissiles",
    "TankHunterTNTAttack",
    "StabAttack",
    "PlaceRemoteCharge",
    "PlaceTimedCharge",
    "Defector",
    "Dock",
    "FireFlame",
    "FireBomb",
    "PlaceBeacon",
    "DisguiseAsVehicle",
    "Waypoint",
    "OutRange",
    "StabAttackInvalid",
    "PlaceChargeInvalid",
    "Hack",
    "ParticleUplinkCannon",
];

/// Retail `NUM_MOUSE_CURSORS` residual (keep-last enum value = name table length).
pub const NUM_MOUSE_CURSORS: usize = 40;

/// Retail `INVALID_MOUSE_CURSOR`.
pub const INVALID_MOUSE_CURSOR: i32 = -1;

/// Retail `RedrawMode` residual names.
pub const MOUSE_REDRAW_MODE_NAMES: &[&str] = &["WINDOWS", "W3D", "POLYGON", "DX8"];
/// Retail `RM_MAX` residual.
pub const MOUSE_REDRAW_MODE_MAX: usize = 4;

/// Click-distance residual: squared distance within click delta.
pub fn mouse_click_distance_ok_residual(dx: i32, dy: i32) -> bool {
    let d2 = (dx as i64) * (dx as i64) + (dy as i64) * (dy as i64);
    d2 <= i64::from(CLICK_DISTANCE_DELTA_SQUARED)
}

// ---------------------------------------------------------------------------
// Keyboard residual
// ---------------------------------------------------------------------------

/// Retail keyboard residual sizes.
pub const KEYBOARD_NUM_KEYS: u32 = 256;
pub const KEYBOARD_KEY_NAMES_COUNT: u32 = 256;
pub const KEYBOARD_MAX_KEY_STATES: u32 = 3;
pub const KEYBOARD_KEY_REPEAT_DELAY: u32 = 10;

/// Retail `KeyboardIO::StatusType` residual.
pub const KEYBOARD_STATUS_UNUSED: u32 = 0x00;
pub const KEYBOARD_STATUS_USED: u32 = 0x01;

// ---------------------------------------------------------------------------
// View residual
// ---------------------------------------------------------------------------

/// Retail default view residual.
pub const DEFAULT_VIEW_WIDTH: u32 = 640;
pub const DEFAULT_VIEW_HEIGHT: u32 = 480;
pub const DEFAULT_VIEW_ORIGIN_X: u32 = 0;
pub const DEFAULT_VIEW_ORIGIN_Y: u32 = 0;

/// Retail `CameraShakeType` residual names.
pub const CAMERA_SHAKE_TYPE_NAMES: &[&str] = &[
    "SUBTLE",
    "NORMAL",
    "STRONG",
    "SEVERE",
    "CINE_EXTREME",
    "CINE_INSANE",
];
/// Retail `SHAKE_COUNT` residual.
pub const CAMERA_SHAKE_COUNT: usize = 6;

/// Retail `WorldToScreenReturn` residual names.
pub const WORLD_TO_SCREEN_RETURN_NAMES: &[&str] = &["INSIDE_FRUSTUM", "OUTSIDE_FRUSTUM", "INVALID"];
/// Retail `WTS_COUNT` residual.
pub const WORLD_TO_SCREEN_RETURN_COUNT: usize = 3;

/// Retail `CameraLockType` residual names.
pub const CAMERA_LOCK_TYPE_NAMES: &[&str] = &["LOCK_FOLLOW", "LOCK_TETHER"];
pub const CAMERA_LOCK_TYPE_COUNT: usize = 2;

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: Mouse constants / cursor table residual pack.
pub fn honesty_mouse_residual_wave112() -> bool {
    MOUSE_BUTTON_STATE_NAMES.len() == MOUSE_BUTTON_STATE_COUNT
        && MOUSE_MOVE_RELATIVE == 0
        && MOUSE_MOVE_ABSOLUTE == 1
        && CLICK_SENSITIVITY == 15
        && CLICK_DISTANCE_DELTA == 10
        && CLICK_DISTANCE_DELTA_SQUARED == 100
        && MOUSE_WHEEL_DELTA == 120
        && MOUSE_OK == 0x01
        && MOUSE_LOST == 0xFF
        && MAX_2D_CURSOR_ANIM_FRAMES == 21
        && MAX_2D_CURSOR_DIRECTIONS == 8
        && NUM_MOUSE_EVENTS == 256
        && INVALID_MOUSE_CURSOR == -1
        && MOUSE_CURSOR_INI_NAMES.len() == NUM_MOUSE_CURSORS
        && residual_name_index(MOUSE_CURSOR_INI_NAMES, "None") == Some(0)
        && residual_name_index(MOUSE_CURSOR_INI_NAMES, "Normal") == Some(1)
        && residual_name_index(MOUSE_CURSOR_INI_NAMES, "ParticleUplinkCannon") == Some(39)
        && residual_name_index(MOUSE_CURSOR_INI_NAMES, "Hack") == Some(38)
        && residual_name_index(MOUSE_CURSOR_INI_NAMES, "NoSuch").is_none()
        && MOUSE_REDRAW_MODE_NAMES.len() == MOUSE_REDRAW_MODE_MAX
        && residual_name_index(MOUSE_REDRAW_MODE_NAMES, "WINDOWS") == Some(0)
        && residual_name_index(MOUSE_REDRAW_MODE_NAMES, "DX8") == Some(3)
        && mouse_click_distance_ok_residual(0, 0)
        && mouse_click_distance_ok_residual(10, 0)
        && !mouse_click_distance_ok_residual(11, 0)
}

/// Honesty: Keyboard residual pack.
pub fn honesty_keyboard_residual_wave112() -> bool {
    KEYBOARD_NUM_KEYS == 256
        && KEYBOARD_KEY_NAMES_COUNT == 256
        && KEYBOARD_MAX_KEY_STATES == 3
        && KEYBOARD_KEY_REPEAT_DELAY == 10
        && KEYBOARD_STATUS_UNUSED == 0
        && KEYBOARD_STATUS_USED == 1
}

/// Honesty: View residual pack.
pub fn honesty_view_residual_wave112() -> bool {
    DEFAULT_VIEW_WIDTH == 640
        && DEFAULT_VIEW_HEIGHT == 480
        && DEFAULT_VIEW_ORIGIN_X == 0
        && DEFAULT_VIEW_ORIGIN_Y == 0
        && CAMERA_SHAKE_TYPE_NAMES.len() == CAMERA_SHAKE_COUNT
        && residual_name_index(CAMERA_SHAKE_TYPE_NAMES, "SUBTLE") == Some(0)
        && residual_name_index(CAMERA_SHAKE_TYPE_NAMES, "CINE_INSANE") == Some(5)
        && WORLD_TO_SCREEN_RETURN_NAMES.len() == WORLD_TO_SCREEN_RETURN_COUNT
        && residual_name_index(WORLD_TO_SCREEN_RETURN_NAMES, "INSIDE_FRUSTUM") == Some(0)
        && residual_name_index(WORLD_TO_SCREEN_RETURN_NAMES, "INVALID") == Some(2)
        && CAMERA_LOCK_TYPE_NAMES.len() == CAMERA_LOCK_TYPE_COUNT
        && residual_name_index(CAMERA_LOCK_TYPE_NAMES, "LOCK_FOLLOW") == Some(0)
        && residual_name_index(CAMERA_LOCK_TYPE_NAMES, "LOCK_TETHER") == Some(1)
}

/// Wave 112 composite residual honesty pack.
pub fn honesty_mouse_keyboard_view_residual_pack_wave112() -> bool {
    honesty_mouse_residual_wave112()
        && honesty_keyboard_residual_wave112()
        && honesty_view_residual_wave112()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_residual() {
        assert!(honesty_mouse_residual_wave112());
    }

    #[test]
    fn keyboard_residual() {
        assert!(honesty_keyboard_residual_wave112());
    }

    #[test]
    fn view_residual() {
        assert!(honesty_view_residual_wave112());
    }

    #[test]
    fn wave112_composite_pack() {
        assert!(honesty_mouse_keyboard_view_residual_pack_wave112());
    }
}
