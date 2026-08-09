//! Wave 111 residual peels: Drawable / Display / GameClient factory residual
//! (host-testable client visual path; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 79 drawable stealth-look save fields, Wave 93 drawable
//! opacity/shroud, Wave 104 drawable create, Wave 107 particle deepen.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - Drawable.h DrawableIconType / MAX_ICONS, StealthLookType, DrawableStatus,
//!   TintStatus, TerrainDecalType, DRAWABLE_FRAMES_PER_FLASH,
//!   DEFAULT_TINT_COLOR_FADE_RATE
//! - Display.h DrawImageMode
//! - GameClient.h MAX_CLIENT_TRANSLATORS
//! - ParticleSys.h ParticlePriorityType / MAX_KEYFRAMES / NUM_PARTICLE_PRIORITIES
//! - GameCommon.h LOGICFRAMES_PER_SECOND = 30
//!
//! Fail-closed:
//! - Not full Drawable draw module chain / stealth material pass residual
//! - Not full Display letterbox / movie residual
//! - Not full GameClient translator stream residual
//! - Not full ParticleSystemManager cap / LOD residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Logic frame residual (shared with Drawable flash timing)
// ---------------------------------------------------------------------------

/// Retail `LOGICFRAMES_PER_SECOND`.
pub const LOGICFRAMES_PER_SECOND: u32 = 30;

/// Retail `DRAWABLE_FRAMES_PER_FLASH` = LOGICFRAMES_PER_SECOND / 2.
pub const DRAWABLE_FRAMES_PER_FLASH: u32 = LOGICFRAMES_PER_SECOND / 2;

/// Retail `DEFAULT_TINT_COLOR_FADE_RATE`.
pub const DEFAULT_TINT_COLOR_FADE_RATE: f32 = 0.6;

// ---------------------------------------------------------------------------
// DrawableIconType residual
// ---------------------------------------------------------------------------

/// Retail `DrawableIconType` residual names (order matches C++ without ALLOW_DEMORALIZE).
pub const DRAWABLE_ICON_TYPE_NAMES: &[&str] = &[
    "DEFAULT_HEAL",
    "STRUCTURE_HEAL",
    "VEHICLE_HEAL",
    "DEMORALIZED_OBSOLETE",
    "BOMB_TIMED",
    "BOMB_REMOTE",
    "DISABLED",
    "BATTLEPLAN_BOMBARD",
    "BATTLEPLAN_HOLDTHELINE",
    "BATTLEPLAN_SEARCHANDDESTROY",
    "EMOTICON",
    "ENTHUSIASTIC",
    "ENTHUSIASTIC_SUBLIMINAL",
    "CARBOMB",
];

/// Retail `MAX_ICONS` residual (keep-last enum value = count of icon slots).
pub const MAX_ICONS: usize = 14;

/// Retail `ICON_INVALID`.
pub const ICON_INVALID: i32 = -1;

// ---------------------------------------------------------------------------
// StealthLookType residual
// ---------------------------------------------------------------------------

/// Retail `StealthLookType` residual names.
pub const STEALTH_LOOK_TYPE_NAMES: &[&str] = &[
    "NONE",
    "VISIBLE_FRIENDLY",
    "DISGUISED_ENEMY",
    "VISIBLE_DETECTED",
    "VISIBLE_FRIENDLY_DETECTED",
    "INVISIBLE",
];

/// Retail stealth-look type count residual.
pub const STEALTH_LOOK_TYPE_COUNT: usize = 6;

// ---------------------------------------------------------------------------
// DrawableStatus / TintStatus bit residual
// ---------------------------------------------------------------------------

/// Retail `DrawableStatus` bits.
pub const DRAWABLE_STATUS_NONE: u32 = 0x0000_0000;
pub const DRAWABLE_STATUS_DRAWS_IN_MIRROR: u32 = 0x0000_0001;
pub const DRAWABLE_STATUS_SHADOWS: u32 = 0x0000_0002;
pub const DRAWABLE_STATUS_TINT_COLOR_LOCKED: u32 = 0x0000_0004;
pub const DRAWABLE_STATUS_NO_STATE_PARTICLES: u32 = 0x0000_0008;
pub const DRAWABLE_STATUS_NO_SAVE: u32 = 0x0000_0010;

/// Retail `TintStatus` bits.
pub const TINT_STATUS_DISABLED: u32 = 0x0000_0001;
pub const TINT_STATUS_IRRADIATED: u32 = 0x0000_0002;
pub const TINT_STATUS_POISONED: u32 = 0x0000_0004;
pub const TINT_STATUS_GAINING_SUBDUAL_DAMAGE: u32 = 0x0000_0008;
pub const TINT_STATUS_FRENZY: u32 = 0x0000_0010;

/// Combine drawable status residual bits.
pub fn drawable_status_combine_residual(bits: &[u32]) -> u32 {
    bits.iter().fold(0u32, |a, b| a | *b)
}

// ---------------------------------------------------------------------------
// TerrainDecalType residual
// ---------------------------------------------------------------------------

/// Retail `TerrainDecalType` residual names (without ALLOW_DEMORALIZE).
pub const TERRAIN_DECAL_TYPE_NAMES: &[&str] = &[
    "DEMORALIZED_OBSOLETE",
    "HORDE",
    "HORDE_WITH_NATIONALISM_UPGRADE",
    "HORDE_VEHICLE",
    "HORDE_WITH_NATIONALISM_UPGRADE_VEHICLE",
    "CRATE",
    "HORDE_WITH_FANATICISM_UPGRADE",
    "CHEMSUIT",
    "NONE",
    "SHADOW_TEXTURE",
];

/// Retail `TERRAIN_DECAL_MAX` residual (keep-last).
pub const TERRAIN_DECAL_MAX: usize = 10;

// ---------------------------------------------------------------------------
// Display DrawImageMode residual
// ---------------------------------------------------------------------------

/// Retail `Display::DrawImageMode` residual names.
pub const DRAW_IMAGE_MODE_NAMES: &[&str] = &["SOLID", "GRAYSCALE", "ALPHA", "ADDITIVE"];

/// Retail draw-image mode count residual.
pub const DRAW_IMAGE_MODE_COUNT: usize = 4;

// ---------------------------------------------------------------------------
// GameClient residual
// ---------------------------------------------------------------------------

/// Retail `GameClient::MAX_CLIENT_TRANSLATORS`.
pub const MAX_CLIENT_TRANSLATORS: u32 = 32;

// ---------------------------------------------------------------------------
// ParticleSystem residual
// ---------------------------------------------------------------------------

/// Retail `ParticleSys.h` `MAX_KEYFRAMES`.
pub const MAX_KEYFRAMES: u32 = 8;

/// Retail `ParticlePriorityType` residual names (INVALID + LOWEST..ALWAYS_RENDER).
pub const PARTICLE_PRIORITY_TYPE_NAMES: &[&str] = &[
    "INVALID_PRIORITY",
    "WEAPON_EXPLOSION",
    "SCORCHMARK",
    "DUST_TRAIL",
    "BUILDUP",
    "DEBRIS_TRAIL",
    "UNIT_DAMAGE_FX",
    "DEATH_EXPLOSION",
    "SEMI_CONSTANT",
    "CONSTANT",
    "WEAPON_TRAIL",
    "AREA_EFFECT",
    "CRITICAL",
    "ALWAYS_RENDER",
];

/// Retail `NUM_PARTICLE_PRIORITIES` residual (keep-last enum value).
pub const NUM_PARTICLE_PRIORITIES: usize = 14;

/// Retail `PARTICLE_PRIORITY_LOWEST` residual ordinal.
pub const PARTICLE_PRIORITY_LOWEST: u32 = 1;
/// Retail `PARTICLE_PRIORITY_HIGHEST` residual ordinal (= ALWAYS_RENDER).
pub const PARTICLE_PRIORITY_HIGHEST: u32 = 13;

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: Drawable icon / flash residual pack.
pub fn honesty_drawable_icon_flash_residual_wave111() -> bool {
    LOGICFRAMES_PER_SECOND == 30
        && DRAWABLE_FRAMES_PER_FLASH == 15
        && (DEFAULT_TINT_COLOR_FADE_RATE - 0.6).abs() < 0.001
        && ICON_INVALID == -1
        && DRAWABLE_ICON_TYPE_NAMES.len() == MAX_ICONS
        && residual_name_index(DRAWABLE_ICON_TYPE_NAMES, "DEFAULT_HEAL") == Some(0)
        && residual_name_index(DRAWABLE_ICON_TYPE_NAMES, "CARBOMB") == Some(13)
        && residual_name_index(DRAWABLE_ICON_TYPE_NAMES, "BOGUS").is_none()
}

/// Honesty: StealthLook / status / tint residual pack.
pub fn honesty_drawable_status_stealth_residual_wave111() -> bool {
    STEALTH_LOOK_TYPE_NAMES.len() == STEALTH_LOOK_TYPE_COUNT
        && residual_name_index(STEALTH_LOOK_TYPE_NAMES, "NONE") == Some(0)
        && residual_name_index(STEALTH_LOOK_TYPE_NAMES, "INVISIBLE") == Some(5)
        && DRAWABLE_STATUS_NONE == 0
        && DRAWABLE_STATUS_SHADOWS == 0x2
        && DRAWABLE_STATUS_NO_SAVE == 0x10
        && drawable_status_combine_residual(&[
            DRAWABLE_STATUS_SHADOWS,
            DRAWABLE_STATUS_NO_STATE_PARTICLES,
        ]) == (0x2 | 0x8)
        && TINT_STATUS_DISABLED == 0x1
        && TINT_STATUS_FRENZY == 0x10
        && TINT_STATUS_POISONED == 0x4
}

/// Honesty: Terrain decal residual pack.
pub fn honesty_terrain_decal_residual_wave111() -> bool {
    TERRAIN_DECAL_TYPE_NAMES.len() == TERRAIN_DECAL_MAX
        && residual_name_index(TERRAIN_DECAL_TYPE_NAMES, "DEMORALIZED_OBSOLETE") == Some(0)
        && residual_name_index(TERRAIN_DECAL_TYPE_NAMES, "NONE") == Some(8)
        && residual_name_index(TERRAIN_DECAL_TYPE_NAMES, "SHADOW_TEXTURE") == Some(9)
}

/// Honesty: Display draw-image mode residual pack.
pub fn honesty_display_draw_image_mode_residual_wave111() -> bool {
    DRAW_IMAGE_MODE_NAMES.len() == DRAW_IMAGE_MODE_COUNT
        && residual_name_index(DRAW_IMAGE_MODE_NAMES, "SOLID") == Some(0)
        && residual_name_index(DRAW_IMAGE_MODE_NAMES, "ADDITIVE") == Some(3)
}

/// Honesty: GameClient translator residual pack.
pub fn honesty_game_client_translator_residual_wave111() -> bool {
    MAX_CLIENT_TRANSLATORS == 32
}

/// Honesty: Particle priority / keyframe residual pack.
pub fn honesty_particle_priority_residual_wave111() -> bool {
    MAX_KEYFRAMES == 8
        && PARTICLE_PRIORITY_TYPE_NAMES.len() == NUM_PARTICLE_PRIORITIES
        && PARTICLE_PRIORITY_LOWEST == 1
        && PARTICLE_PRIORITY_HIGHEST == 13
        && residual_name_index(PARTICLE_PRIORITY_TYPE_NAMES, "INVALID_PRIORITY") == Some(0)
        && residual_name_index(PARTICLE_PRIORITY_TYPE_NAMES, "WEAPON_EXPLOSION") == Some(1)
        && residual_name_index(PARTICLE_PRIORITY_TYPE_NAMES, "ALWAYS_RENDER") == Some(13)
        && residual_name_index(PARTICLE_PRIORITY_TYPE_NAMES, "CRITICAL") == Some(12)
}

/// Wave 111 composite residual honesty pack.
pub fn honesty_drawable_display_client_residual_pack_wave111() -> bool {
    honesty_drawable_icon_flash_residual_wave111()
        && honesty_drawable_status_stealth_residual_wave111()
        && honesty_terrain_decal_residual_wave111()
        && honesty_display_draw_image_mode_residual_wave111()
        && honesty_game_client_translator_residual_wave111()
        && honesty_particle_priority_residual_wave111()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawable_icon_flash() {
        assert!(honesty_drawable_icon_flash_residual_wave111());
    }

    #[test]
    fn drawable_status_stealth() {
        assert!(honesty_drawable_status_stealth_residual_wave111());
    }

    #[test]
    fn terrain_decal() {
        assert!(honesty_terrain_decal_residual_wave111());
    }

    #[test]
    fn display_draw_image_mode() {
        assert!(honesty_display_draw_image_mode_residual_wave111());
    }

    #[test]
    fn game_client_translators() {
        assert!(honesty_game_client_translator_residual_wave111());
    }

    #[test]
    fn particle_priority() {
        assert!(honesty_particle_priority_residual_wave111());
    }

    #[test]
    fn wave111_composite_pack() {
        assert!(honesty_drawable_display_client_residual_pack_wave111());
    }
}
