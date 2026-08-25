//! Host GLA Bomb Truck disguise residual.
//!
//! Residual slice (playability):
//! - `GLAVehicleBombTruck` (and Demo_/Chem_/Slth_ / Boss_ variants) issues
//!   `SpecialAbilityDisguiseAsVehicle` / `DisguiseAsVehicle` on a legal vehicle
//!   target → sets OBJECT_STATUS_DISGUISED + STEALTHED residual, stores the
//!   target's template + team as the disguise appearance residual
//!   (C++ StealthUpdate::disguiseAsTemplate + DisguisesAsTeam = Yes).
//! - Enemies of the bomb truck see the **disguise team** for relationship /
//!   auto-target residual (do not auto-attack when disguised as their ally).
//! - C++ RevealDistanceFromTarget = 100: while attacking, if distance to
//!   current victim ≤ 100, reveal (clear DISGUISED + STEALTHED residual).
//! - Attacking / force-fire residual also reveals (OrderIdleEnemies path is
//!   fail-closed to clear-disguise only).
//!
//! Wave 66 residual pack (retail GLAVehicle.ini / SpecialPower.ini / Locomotor.ini):
//! - Disguise residual: SpecialAbilityDisguiseAsVehicle, DisguisesAsTeam **Yes**,
//!   RevealDistanceFromTarget **100**, TransitionTime **2000**ms → **60**f,
//!   RevealTransitionTime **1000**ms → **30**f, FX_BombTruckDisguise /
//!   FX_BombTruckDisguiseReveal.
//! - Body residual: MaxHealth **220**, Vision **150**, Shroud **200**,
//!   BuildCost **1200**, BuildTime **15**s → **450**f, TransportSlotCount **3**,
//!   BombTruckLocomotor Speed **50**/Damaged **50**.
//!
//! Fail-closed honesty:
//! - Not full StealthUpdate transition opacity / half-point model swap
//! - Not full drawable indicator-color night/day matrix for disguised players
//! - Not full academy stats on disguise
//! - Not full radar / selection portrait swap to disguise template art
//! - Not network disguise replication (network deferred)

use super::{ObjectId, Team};
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const BOMB_TRUCK_LOGIC_FPS: f32 = 30.0;

/// Retail special-power template residual.
pub const BOMB_TRUCK_DISGUISE_SPECIAL_POWER: &str = "SpecialAbilityDisguiseAsVehicle";
/// Retail special-power enum residual.
pub const BOMB_TRUCK_DISGUISE_ENUM: &str = "SPECIAL_DISGUISE_AS_VEHICLE";

/// C++ StealthUpdate RevealDistanceFromTarget residual (Bomb Truck INI).
pub const BOMB_TRUCK_DISGUISE_REVEAL_DISTANCE: f32 = 100.0;
/// Retail DisguisesAsTeam residual.
pub const BOMB_TRUCK_DISGUISES_AS_TEAM: bool = true;
/// Retail DisguiseTransitionTime residual (msec).
pub const BOMB_TRUCK_DISGUISE_TRANSITION_MS: u32 = 2000;
/// DisguiseTransitionTime 2000ms → 60 frames @ 30 FPS.
pub const BOMB_TRUCK_DISGUISE_TRANSITION_FRAMES: u32 = 60;
/// Retail DisguiseRevealTransitionTime residual (msec).
pub const BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_MS: u32 = 1000;
/// DisguiseRevealTransitionTime 1000ms → 30 frames @ 30 FPS.
pub const BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_FRAMES: u32 = 30;
/// Retail DisguiseFX residual.
pub const BOMB_TRUCK_DISGUISE_FX: &str = "FX_BombTruckDisguise";
/// Retail DisguiseRevealFX residual.
pub const BOMB_TRUCK_DISGUISE_REVEAL_FX: &str = "FX_BombTruckDisguiseReveal";

/// Audio residual when disguise is applied (Voice.ini BombTruckVoiceDisguise).
pub const BOMB_TRUCK_DISGUISE_AUDIO: &str = "BombTruckVoiceDisguise";

/// Audio residual when disguise is revealed (FX_BombTruckDisguiseReveal residual cue).
pub const BOMB_TRUCK_DISGUISE_REVEAL_AUDIO: &str = "BombTruckVoiceModeDisguise";
/// C++ per-unit sound at disguise halfpoint (`changeVisualDisguise`).
pub const BOMB_TRUCK_DISGUISE_STARTED_AUDIO: &str = "DisguiseStarted";
/// C++ reveal halfpoint when `getCurrentVictim` exists.
pub const BOMB_TRUCK_DISGUISE_REVEALED_SUCCESS_AUDIO: &str = "DisguiseRevealedSuccess";
/// C++ reveal halfpoint when there is no current victim.
pub const BOMB_TRUCK_DISGUISE_REVEALED_FAILURE_AUDIO: &str = "DisguiseRevealedFailure";

// --- Body residual (GLAVehicleBombTruck) ---

/// Retail ActiveBody MaxHealth residual.
pub const BOMB_TRUCK_MAX_HEALTH: f32 = 220.0;
/// Retail VisionRange residual.
pub const BOMB_TRUCK_VISION_RANGE: f32 = 150.0;
/// Retail ShroudClearingRange residual.
pub const BOMB_TRUCK_SHROUD_CLEARING_RANGE: f32 = 200.0;
/// Retail BuildCost residual.
pub const BOMB_TRUCK_BUILD_COST: u32 = 1200;
/// Retail BuildTime residual (seconds).
pub const BOMB_TRUCK_BUILD_TIME_SEC: f32 = 15.0;
/// BuildTime 15s → 450 frames @ 30 FPS.
pub const BOMB_TRUCK_BUILD_TIME_FRAMES: u32 = 450;
/// Retail TransportSlotCount residual.
pub const BOMB_TRUCK_TRANSPORT_SLOT_COUNT: u32 = 3;
/// Retail BombTruckLocomotor Speed residual.
pub const BOMB_TRUCK_LOCOMOTOR_SPEED: f32 = 50.0;
/// Retail BombTruckLocomotor SpeedDamaged residual.
pub const BOMB_TRUCK_LOCOMOTOR_SPEED_DAMAGED: f32 = 50.0;

/// Convert residual milliseconds to logic frames @ 30 FPS (round half-up).
pub fn bomb_truck_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * BOMB_TRUCK_LOGIC_FPS / 1000.0).round() as u32
}

/// Normalize template / name residual matching.
fn alnum_lower(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Whether template is a residual bomb truck (disguise caster).
///
/// Fail-closed: name residual (not full KINDOF_DISGUISER matrix).
pub fn is_bomb_truck_template(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.is_empty() {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testbombtruck" || n == "test_bomb_truck" {
        return true;
    }
    n.contains("bombtruck")
}

/// Exact retail source identities whose `StealthUpdate` declares
/// `DisguisesAsTeam = Yes`.
///
/// This deliberately is narrower than [`is_bomb_truck_template`]. The latter
/// is a gameplay-facing Bomb Truck heuristic and also matches non-disguising
/// assets such as `DeadBombTruckHulk`; the C++ scene-stealth rule instead
/// reads the current template's `StealthUpdate::m_teamDisguised` capability.
/// Normalize the authored identity so case/punctuation-only test fixtures do
/// not create a second behavior class, but fail closed for every other name.
pub fn has_disguises_as_team_stealth_residual(template_name: &str) -> bool {
    matches!(
        alnum_lower(template_name).as_str(),
        "glavehiclebombtruck"
            | "chemglavehiclebombtruck"
            | "demoglavehiclebombtruck"
            | "gcchemglavehiclebombtruck"
            | "cineglavehiclebombtruck"
            | "slthglavehiclebombtruck"
    )
}

/// Whether a template is a legal disguise target residual.
///
/// C++ `ActionManager::canDoSpecialPowerAtObject` `SPECIAL_DISGUISE_AS_VEHICLE`:
/// vehicle && !aircraft && !boat && !cliff_jumper && no `RailroadBehavior`.
/// The same-template bomb-truck reject is commented out in retail, so bomb
/// trucks are legal disguise targets.
pub fn is_legal_disguise_target_template(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.is_empty() {
        return false;
    }
    // C++ `RailroadBehavior` module reject (no KINDOF_TRAIN).
    if n.contains("train") {
        return false;
    }
    // C++ `KINDOF_BOAT` — KindOf bit, INI token, or name residual.
    if n.contains("boat")
        || n.contains("battleship")
        || crate::game_logic::host_car_bomb::object_definition_has_kind(template_name, "BOAT")
    {
        return false;
    }
    // C++ `KINDOF_CLIFF_JUMPER` (same ActionManager conjunction).
    if n.contains("combatbike")
        || n.contains("cliffjumper")
        || crate::game_logic::host_car_bomb::object_definition_has_kind(
            template_name,
            "CLIFF_JUMPER",
        )
    {
        return false;
    }
    true
}

/// Runtime legal target residual (kind + status).
///
/// `is_boat` is C++ `KINDOF_BOAT`. `is_already_disguised` is unused by the
/// ActionManager gate (retail does not special-case DISGUISED here).
pub fn is_legal_disguise_target(
    is_alive: bool,
    is_vehicle: bool,
    is_aircraft: bool,
    is_boat: bool,
    template_name: &str,
    _is_already_disguised: bool,
) -> bool {
    if !is_alive || !is_vehicle || is_aircraft || is_boat {
        return false;
    }
    is_legal_disguise_target_template(template_name)
}

/// Whether reveal-distance residual triggers (2D distance).
pub fn should_reveal_disguise_by_distance(distance: f32) -> bool {
    distance <= BOMB_TRUCK_DISGUISE_REVEAL_DISTANCE
}

/// Apparent team residual: non-allied viewers see the disguise team.
///
/// C++ Player::getRelationship color / selection residual:
/// Neutrals and enemies see the unit as the team it's disguised as.
/// Allies of the real owner still see the real team.
pub fn apparent_team_for_viewer(
    real_team: Team,
    disguise_team: Option<Team>,
    is_disguised: bool,
    viewer_team: Team,
) -> Team {
    if !is_disguised {
        return real_team;
    }
    let Some(disguise) = disguise_team else {
        return real_team;
    };
    // Ally of real owner: see through disguise residual.
    if viewer_team == real_team {
        return real_team;
    }
    disguise
}

/// Whether `attacker_team` should auto-target a unit based on apparent team residual.
/// Returns true when the apparent team is an enemy of the attacker.
pub fn is_auto_targetable_as_enemy(
    real_team: Team,
    disguise_team: Option<Team>,
    is_disguised: bool,
    attacker_team: Team,
) -> bool {
    if attacker_team == Team::Neutral {
        return false;
    }
    let apparent = apparent_team_for_viewer(real_team, disguise_team, is_disguised, attacker_team);
    apparent != attacker_team && apparent != Team::Neutral
}

/// Host residual honesty counters for bomb-truck disguise.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostBombTruckDisguiseRegistry {
    /// Successful disguise applications.
    pub disguises: u32,
    /// C++ disguiseAsObject copy from already-disguised target.
    pub disguise_copies: u32,
    /// Disguise transition residual starts (apply or reveal arm).
    pub transition_starts: u32,
    /// Halfpoint changeVisualDisguise residual fires.
    pub transition_halfpoints: u32,
    /// Successful disguise reveals (distance / attack residual).
    pub reveals: u32,
    /// Last disguised object id (residual observability).
    pub last_disguised_id: Option<ObjectId>,
    /// Last disguise template name residual.
    pub last_disguise_template: Option<String>,
}

impl HostBombTruckDisguiseRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_disguise(&mut self, object_id: ObjectId, template_name: &str) {
        self.disguises = self.disguises.saturating_add(1);
        self.last_disguised_id = Some(object_id);
        self.last_disguise_template = Some(template_name.to_string());
    }

    pub fn record_disguise_copy(&mut self) {
        self.disguise_copies = self.disguise_copies.saturating_add(1);
    }

    /// Residual honesty: copied disguise from an already-disguised target.
    pub fn honesty_disguise_copy_ok(&self) -> bool {
        self.disguise_copies > 0
    }

    pub fn record_transition_start(&mut self) {
        self.transition_starts = self.transition_starts.saturating_add(1);
    }

    pub fn record_transition_halfpoint(&mut self) {
        self.transition_halfpoints = self.transition_halfpoints.saturating_add(1);
    }

    pub fn honesty_transition_halfpoint_ok(&self) -> bool {
        self.transition_halfpoints > 0
    }

    pub fn record_reveal(&mut self) {
        self.reveals = self.reveals.saturating_add(1);
    }

    /// Residual honesty: at least one disguise applied.
    pub fn honesty_disguise_ok(&self) -> bool {
        self.disguises > 0
    }

    /// Residual honesty: at least one reveal resolved.
    pub fn honesty_reveal_ok(&self) -> bool {
        self.reveals > 0
    }

    /// Combined residual path honesty (disguise required; reveal optional polish).
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_disguise_ok()
    }
}

// --- Wave 66 residual honesty packs ---

/// Wave 66 residual honesty: disguise special-power / stealth residual peel.
pub fn honesty_bomb_truck_disguise_ability_residual_ok() -> bool {
    BOMB_TRUCK_DISGUISE_SPECIAL_POWER == "SpecialAbilityDisguiseAsVehicle"
        && BOMB_TRUCK_DISGUISE_ENUM == "SPECIAL_DISGUISE_AS_VEHICLE"
        && (BOMB_TRUCK_DISGUISE_REVEAL_DISTANCE - 100.0).abs() < 0.01
        && BOMB_TRUCK_DISGUISES_AS_TEAM
        && BOMB_TRUCK_DISGUISE_TRANSITION_MS == 2000
        && BOMB_TRUCK_DISGUISE_TRANSITION_FRAMES
            == bomb_truck_ms_to_frames(BOMB_TRUCK_DISGUISE_TRANSITION_MS)
        && BOMB_TRUCK_DISGUISE_TRANSITION_FRAMES == 60
        && BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_MS == 1000
        && BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_FRAMES
            == bomb_truck_ms_to_frames(BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_MS)
        && BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_FRAMES == 30
        && BOMB_TRUCK_DISGUISE_FX == "FX_BombTruckDisguise"
        && BOMB_TRUCK_DISGUISE_REVEAL_FX == "FX_BombTruckDisguiseReveal"
        && BOMB_TRUCK_DISGUISE_AUDIO == "BombTruckVoiceDisguise"
        && BOMB_TRUCK_DISGUISE_STARTED_AUDIO == "DisguiseStarted"
        && BOMB_TRUCK_DISGUISE_REVEALED_SUCCESS_AUDIO == "DisguiseRevealedSuccess"
        && BOMB_TRUCK_DISGUISE_REVEALED_FAILURE_AUDIO == "DisguiseRevealedFailure"
        && should_reveal_disguise_by_distance(100.0)
        && !should_reveal_disguise_by_distance(100.1)
}

/// Wave 66 residual honesty: bomb truck body residual peel.
pub fn honesty_bomb_truck_body_residual_ok() -> bool {
    (BOMB_TRUCK_MAX_HEALTH - 220.0).abs() < 0.01
        && (BOMB_TRUCK_VISION_RANGE - 150.0).abs() < 0.01
        && (BOMB_TRUCK_SHROUD_CLEARING_RANGE - 200.0).abs() < 0.01
        && BOMB_TRUCK_BUILD_COST == 1200
        && (BOMB_TRUCK_BUILD_TIME_SEC - 15.0).abs() < 0.01
        && BOMB_TRUCK_BUILD_TIME_FRAMES
            == ((BOMB_TRUCK_BUILD_TIME_SEC * BOMB_TRUCK_LOGIC_FPS).round() as u32)
        && BOMB_TRUCK_BUILD_TIME_FRAMES == 450
        && BOMB_TRUCK_TRANSPORT_SLOT_COUNT == 3
        && (BOMB_TRUCK_LOCOMOTOR_SPEED - 50.0).abs() < 0.01
        && (BOMB_TRUCK_LOCOMOTOR_SPEED_DAMAGED - 50.0).abs() < 0.01
        && is_bomb_truck_template("GLAVehicleBombTruck")
        && !is_bomb_truck_template("GLAVehicleQuadCannon")
}

/// Combined Wave 66 Bomb Truck disguise residual honesty pack.
pub fn honesty_bomb_truck_disguise_residual_pack_ok() -> bool {
    honesty_bomb_truck_disguise_ability_residual_ok() && honesty_bomb_truck_body_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bomb_truck_name_residual() {
        assert!(is_bomb_truck_template("GLAVehicleBombTruck"));
        assert!(is_bomb_truck_template("Demo_GLAVehicleBombTruck"));
        assert!(is_bomb_truck_template("TestBombTruck"));
        assert!(!is_bomb_truck_template("GLAVehicleQuadCannon"));
        assert!(!is_bomb_truck_template("USA_Ranger"));
    }

    #[test]
    fn only_authored_team_disguise_templates_receive_the_scene_capability() {
        for name in [
            "GLAVehicleBombTruck",
            "Chem_GLAVehicleBombTruck",
            "Demo_GLAVehicleBombTruck",
            "GC_Chem_GLAVehicleBombTruck",
            "CINE_GLAVehicleBombTruck",
            "Slth_GLAVehicleBombTruck",
        ] {
            assert!(has_disguises_as_team_stealth_residual(name), "{name}");
        }
        assert!(!has_disguises_as_team_stealth_residual("DeadBombTruckHulk"));
        assert!(!has_disguises_as_team_stealth_residual("CustomBombTruck"));
    }

    #[test]
    fn legal_disguise_target_allows_bomb_truck_rejects_boats() {
        assert!(is_legal_disguise_target(
            true,
            true,
            false,
            false,
            "AmericaTankCrusader",
            false
        ));
        // C++ bomb-truck same-template reject is commented out.
        assert!(is_legal_disguise_target(
            true,
            true,
            false,
            false,
            "GLAVehicleBombTruck",
            false
        ));
        assert!(is_legal_disguise_target_template("GLAVehicleBombTruck"));
        assert!(!is_legal_disguise_target(
            true,
            true,
            true,
            false,
            "AmericaJetRaptor",
            false
        ));
        assert!(!is_legal_disguise_target(
            false,
            true,
            false,
            false,
            "AmericaTankCrusader",
            false
        ));
        assert!(!is_legal_disguise_target(
            true,
            true,
            false,
            true,
            "AmericaVehicleBattleship",
            false
        ));
        assert!(!is_legal_disguise_target_template("CivilianFishingBoat"));
        assert!(!is_legal_disguise_target_template("CivilianTrainEngine"));
    }

    #[test]
    fn apparent_team_enemies_see_disguise() {
        assert_eq!(
            apparent_team_for_viewer(Team::GLA, Some(Team::USA), true, Team::China),
            Team::USA
        );
        // Ally of real owner sees through.
        assert_eq!(
            apparent_team_for_viewer(Team::GLA, Some(Team::USA), true, Team::GLA),
            Team::GLA
        );
        assert_eq!(
            apparent_team_for_viewer(Team::GLA, Some(Team::USA), false, Team::China),
            Team::GLA
        );
    }

    #[test]
    fn auto_target_skips_disguised_as_attacker_team() {
        // GLA bomb truck disguised as USA: USA attackers should not auto-target.
        assert!(!is_auto_targetable_as_enemy(
            Team::GLA,
            Some(Team::USA),
            true,
            Team::USA
        ));
        // China still sees it as USA enemy → auto-target ok.
        assert!(is_auto_targetable_as_enemy(
            Team::GLA,
            Some(Team::USA),
            true,
            Team::China
        ));
        // Undisguised GLA is enemy of USA.
        assert!(is_auto_targetable_as_enemy(
            Team::GLA,
            None,
            false,
            Team::USA
        ));
    }

    #[test]
    fn reveal_distance_residual() {
        assert!(should_reveal_disguise_by_distance(0.0));
        assert!(should_reveal_disguise_by_distance(100.0));
        assert!(!should_reveal_disguise_by_distance(100.1));
    }

    #[test]
    fn honesty_registry() {
        let mut reg = HostBombTruckDisguiseRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        reg.record_disguise(ObjectId(1), "AmericaTankCrusader");
        assert!(reg.honesty_disguise_ok());
        assert!(reg.honesty_host_path_ok());
        assert!(!reg.honesty_reveal_ok());
        reg.record_reveal();
        assert!(reg.honesty_reveal_ok());
    }

    #[test]
    fn bomb_truck_disguise_residual_pack_honesty_wave66() {
        assert_eq!(bomb_truck_ms_to_frames(2000), 60);
        assert_eq!(bomb_truck_ms_to_frames(1000), 30);
        assert!(honesty_bomb_truck_disguise_ability_residual_ok());
        assert!(honesty_bomb_truck_body_residual_ok());
        assert!(honesty_bomb_truck_disguise_residual_pack_ok());
        assert!(BOMB_TRUCK_DISGUISES_AS_TEAM);
        assert_eq!(BOMB_TRUCK_BUILD_TIME_FRAMES, 450);
        assert_eq!(
            BOMB_TRUCK_DISGUISE_SPECIAL_POWER,
            "SpecialAbilityDisguiseAsVehicle"
        );
    }
}
