//! Host Colonel Burton residual combat polish (sniper rifle + knife melee).
//!
//! Residual slice (playability):
//! - `AmericaInfantryColonelBurton` / SupW_/CINE_ variants spawn with PRIMARY
//!   `ColonelBurtonSniperRifleWeapon` (dmg **40** / range **125** / Delay **100**ms
//!   → 3 frames). ClipSize **3** honesty (ClipReload 500ms fail-closed volley matrix).
//! - Knife residual (`ColonelBurtonKnifeWeapon`): close-range infantry within **3**
//!   → MELEE one-shot PrimaryDamage **10000** (LeechRangeWeapon residual).
//! - Timed / remote demo charges already closed via host_mines / host_hero_abilities
//!   (not re-opened).
//!
//! Fail-closed honesty:
//! - Not full ClipSize=3 in-clip DelayBetweenShots + ClipReload 500ms volley matrix
//! - Not full knife PreAttackDelay 833ms / PER_ATTACK anim lock matrix
//! - Not full StealthUpdate / ChemicalSuits / AdvancedTraining residual matrix
//! - Not network sniper / knife replication (network deferred)

use super::Weapon;
use crate::game_logic::host_red_guard::delay_frames_to_reload_secs;

/// Retail primary sniper weapon.
pub const BURTON_SNIPER_WEAPON: &str = "ColonelBurtonSniperRifleWeapon";
/// Retail secondary knife weapon.
pub const BURTON_KNIFE_WEAPON: &str = "ColonelBurtonKnifeWeapon";

/// Retail PrimaryDamage base (sniper).
pub const BURTON_SNIPER_DAMAGE: f32 = 40.0;
/// Retail AttackRange (sniper).
pub const BURTON_SNIPER_RANGE: f32 = 125.0;
/// Retail DelayBetweenShots 100ms → 3 frames @ 30 FPS.
pub const BURTON_SNIPER_BASE_DELAY_FRAMES: u32 = 3;
/// Retail ClipSize residual honesty (fail-closed volley matrix).
pub const BURTON_CLIP_SIZE: u32 = 3;
/// Retail ClipReloadTime 500ms → 15 frames @ 30 FPS (honesty only).
pub const BURTON_CLIP_RELOAD_FRAMES: u32 = 15;

/// Knife PrimaryDamage residual (one-shot kill).
pub const BURTON_KNIFE_DAMAGE: f32 = 10_000.0;
/// Knife AttackRange residual.
pub const BURTON_KNIFE_RANGE: f32 = 3.0;
/// Knife ClipReloadTime 1367ms → 41 frames @ 30 FPS.
pub const BURTON_KNIFE_DELAY_FRAMES: u32 = 41;
/// Knife PreAttackDelay 833ms residual (fail-closed vs full pre-attack lock).
pub const BURTON_KNIFE_PRE_ATTACK_FRAMES: u32 = 25;

/// Residual sniper fire audio.
pub const BURTON_SNIPER_FIRE_AUDIO: &str = "SentryDroneWeapon";
/// Residual knife fire audio.
pub const BURTON_KNIFE_FIRE_AUDIO: &str = "HeroUSAKnifeAttack";

/// Whether template is a residual Colonel Burton hero.
///
/// Fail-closed: name residual. Excludes weapons / science / debris tokens.
pub fn is_colonel_burton_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("command")
        || n.contains("button")
        || n.contains("portrait")
        || n.contains("charge")
        || n.contains("demo")
        || n.contains("plant")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testburton"
        || n == "testcolonelburton"
        || n == "colonel_burton"
        || n == "usa_burton"
        || n == "usa_colonelburton"
    {
        return true;
    }
    n.contains("colonelburton") || n.contains("colonel_burton") || n.contains("burton")
}

/// Sniper delay frames residual (base DelayBetweenShots; clip reload fail-closed).
pub fn burton_sniper_delay_frames() -> u32 {
    BURTON_SNIPER_BASE_DELAY_FRAMES
}

/// (damage, range, delay_frames) for sniper residual.
pub fn burton_sniper_weapon_stats() -> (f32, f32, u32) {
    (
        BURTON_SNIPER_DAMAGE,
        BURTON_SNIPER_RANGE,
        burton_sniper_delay_frames(),
    )
}

/// Build residual PRIMARY sniper Weapon.
pub fn burton_sniper_weapon() -> Weapon {
    let (damage, range, delay) = burton_sniper_weapon_stats();
    Weapon {
        damage,
        range,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: Some(BURTON_CLIP_SIZE),
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
    }
}

/// Residual knife Weapon (close-range one-shot).
pub fn burton_knife_weapon() -> Weapon {
    Weapon {
        damage: BURTON_KNIFE_DAMAGE,
        range: BURTON_KNIFE_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(BURTON_KNIFE_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: Some(1),
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: delay_frames_to_reload_secs(BURTON_KNIFE_PRE_ATTACK_FRAMES),
    }
}

/// Whether knife residual should apply for this shot.
///
/// Residual: target is living infantry, horizontal distance ≤ BURTON_KNIFE_RANGE.
pub fn should_apply_burton_knife_residual(
    is_burton: bool,
    target_is_infantry: bool,
    target_alive: bool,
    distance: f32,
) -> bool {
    is_burton
        && target_is_infantry
        && target_alive
        && distance <= BURTON_KNIFE_RANGE
        && distance >= 0.0
}

/// 2D distance residual (XZ plane).
pub fn distance_2d(ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    let dx = ax - bx;
    let dz = az - bz;
    (dx * dx + dz * dz).sqrt()
}

/// Whether residual fire should apply Burton residual path (sniper/knife honesty).
pub fn should_apply_burton_residual(is_burton: bool) -> bool {
    is_burton
}

/// Legal residual fire target.
pub fn is_legal_burton_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burton_name_matrix() {
        assert!(is_colonel_burton_template("AmericaInfantryColonelBurton"));
        assert!(is_colonel_burton_template("SupW_AmericaInfantryColonelBurton"));
        assert!(is_colonel_burton_template("CINE_AmericaInfantryColonelBurton"));
        assert!(is_colonel_burton_template("TestBurton"));
        assert!(is_colonel_burton_template("USA_ColonelBurton"));
        assert!(!is_colonel_burton_template("ColonelBurtonSniperRifleWeapon"));
        assert!(!is_colonel_burton_template("ColonelBurtonKnifeWeapon"));
        assert!(!is_colonel_burton_template("ColonelBurtonSetDemoCharge"));
        assert!(!is_colonel_burton_template("ColonelBurtonSetRemoteCharge"));
        assert!(!is_colonel_burton_template("ColonelBurtonGroundLocomotor"));
        assert!(!is_colonel_burton_template("AmericaInfantryRanger"));
        assert!(!is_colonel_burton_template("ChinaInfantryBlackLotus"));
    }

    #[test]
    fn sniper_stats() {
        let (d, r, f) = burton_sniper_weapon_stats();
        assert!((d - 40.0).abs() < 0.01);
        assert!((r - 125.0).abs() < 0.01);
        assert_eq!(f, 3);
        let w = burton_sniper_weapon();
        assert!((w.damage - 40.0).abs() < 0.01);
        assert!((w.reload_time - (3.0 / 30.0)).abs() < 0.01);
        assert_eq!(w.ammo, Some(3));
        assert!(!w.can_target_air && w.can_target_ground);
    }

    #[test]
    fn knife_stats_and_gate() {
        let k = burton_knife_weapon();
        assert!((k.damage - 10_000.0).abs() < 0.1);
        assert!((k.range - 3.0).abs() < 0.01);
        assert!(should_apply_burton_knife_residual(true, true, true, 2.5));
        assert!(should_apply_burton_knife_residual(true, true, true, 3.0));
        assert!(!should_apply_burton_knife_residual(true, true, true, 3.1));
        assert!(!should_apply_burton_knife_residual(true, false, true, 1.0));
        assert!(!should_apply_burton_knife_residual(false, true, true, 1.0));
        assert!(!should_apply_burton_knife_residual(true, true, false, 1.0));
    }

    #[test]
    fn legal_and_apply() {
        assert!(should_apply_burton_residual(true));
        assert!(!should_apply_burton_residual(false));
        assert!(is_legal_burton_target(true, false, false, true));
        assert!(!is_legal_burton_target(false, false, false, true));
        assert!(!is_legal_burton_target(true, true, false, true));
    }
}
