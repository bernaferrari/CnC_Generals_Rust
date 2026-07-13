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
//! Fail-closed honesty:
//! - Not full StealthUpdate transition opacity / half-point model swap
//! - Not full drawable indicator-color night/day matrix for disguised players
//! - Not full academy stats / subobject upgrade restore on disguise
//! - Not full radar / selection portrait swap to disguise template art
//! - Not network disguise replication (network deferred)

use super::{ObjectId, Team};
use serde::{Deserialize, Serialize};

/// C++ StealthUpdate RevealDistanceFromTarget residual (Bomb Truck INI).
pub const BOMB_TRUCK_DISGUISE_REVEAL_DISTANCE: f32 = 100.0;

/// Audio residual when disguise is applied (Voice.ini BombTruckVoiceDisguise).
pub const BOMB_TRUCK_DISGUISE_AUDIO: &str = "BombTruckVoiceDisguise";

/// Audio residual when disguise is revealed (FX_BombTruckDisguiseReveal residual cue).
pub const BOMB_TRUCK_DISGUISE_REVEAL_AUDIO: &str = "BombTruckVoiceModeDisguise";

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

/// Whether a template is a legal disguise target residual.
///
/// C++ ActionManager SPECIAL_DISGUISE_AS_VEHICLE:
/// - Must be vehicle (ground residual; aircraft rejected)
/// - Not another bomb truck ("that's just plain dumb")
/// - Not a train (no KINDOF_TRAIN residual — name skip)
pub fn is_legal_disguise_target_template(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.is_empty() {
        return false;
    }
    if is_bomb_truck_template(template_name) {
        return false;
    }
    if n.contains("train") {
        return false;
    }
    true
}

/// Runtime legal target residual (kind + status).
pub fn is_legal_disguise_target(
    is_alive: bool,
    is_vehicle: bool,
    is_aircraft: bool,
    is_bomb_truck: bool,
    template_name: &str,
) -> bool {
    if !is_alive || !is_vehicle || is_aircraft || is_bomb_truck {
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
    fn legal_disguise_target_rejects_bomb_truck_and_aircraft() {
        assert!(is_legal_disguise_target(
            true,
            true,
            false,
            false,
            "AmericaTankCrusader"
        ));
        assert!(!is_legal_disguise_target(
            true,
            true,
            false,
            true,
            "GLAVehicleBombTruck"
        ));
        assert!(!is_legal_disguise_target(
            true,
            true,
            true,
            false,
            "AmericaJetRaptor"
        ));
        assert!(!is_legal_disguise_target(
            false,
            true,
            false,
            false,
            "AmericaTankCrusader"
        ));
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
        assert!(is_auto_targetable_as_enemy(Team::GLA, None, false, Team::USA));
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
}
