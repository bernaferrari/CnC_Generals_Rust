//! Host USA Pilot residual (eject recrew of unmanned vehicles + EjectPilotDie).
//!
//! Residual slice (playability):
//! - `AmericaInfantryPilot` / AirF_ / CINE_ / TestPilot enter unmanned ground
//!   vehicles (DISABLED_UNMANNED residual from Jarmen Kell snipe / neutron) →
//!   recrew: clear unmanned, transfer team to pilot's team, transfer pilot
//!   veterancy (retail `VeterancyCrateCollide` IsPilot + AddsOwnerVeterancy),
//!   consume pilot.
//! - Pilots spawn residual at least VETERAN (VeterancyGainCreate StartingLevel).
//! - **EjectPilotDie residual**: eligible USA ground vehicles (Humvee / Tomahawk /
//!   Crusader / Paladin / Avenger / Microwave + general variants) spawn
//!   `AmericaInfantryPilot` on death via OCL_EjectPilotOnGround residual path.
//!   Fail-closed: unmanned vehicles do not eject (no pilot left).
//!
//! Fail-closed honesty:
//! - Not full EjectPilotDie air OCL parachute / isSignificantlyAboveTerrain matrix
//! - Not full PilotFindVehicleUpdate AI auto-scan / MinHealth enter matrix
//! - Not full AutoFindHealingUpdate hospital path residual
//! - Not full InvulnerableTime post-eject invulnerability matrix
//! - Not network recrew / pilot-eject replication (network deferred)

use super::VeterancyLevel;
use serde::{Deserialize, Serialize};

/// Retail pilot template family residual.
pub const PILOT_RECREW_AUDIO: &str = "PilotEnterVehicle";

/// Retail OCL_EjectPilotOnGround / OCL_EjectPilotViaParachute ObjectNames residual.
pub const EJECT_PILOT_TEMPLATE: &str = "AmericaInfantryPilot";

/// Residual eject audio (VoiceEject / SoundEject fail-closed host cue).
pub const PILOT_EJECT_AUDIO: &str = "VoiceEject";

/// Whether template is a residual USA Pilot infantry.
///
/// Fail-closed: name residual. Excludes weapons / science / debris / pathfinder.
pub fn is_pilot_template(template_name: &str) -> bool {
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
        || n.contains("pathfinder")
        || n.contains("ranger")
        || n.contains("colonel")
        || n.contains("burton")
        || n.contains("commandset")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testpilot" || n == "usa_pilot" || n == "americainfantrypilot" {
        return true;
    }
    // AmericaInfantryPilot / AirF_AmericaInfantryPilot / CINE_AmericaInfantryPilot
    n.contains("infantrypilot") || n.ends_with("pilot") && n.contains("america")
}

/// Residual pilot starting veterancy (VeterancyGainCreate StartingLevel = VETERAN).
pub fn pilot_default_veterancy() -> VeterancyLevel {
    VeterancyLevel::Veteran
}

/// Whether template is a residual USA vehicle with EjectPilotDie module.
///
/// Retail AmericaVehicle.ini / general variants: Humvee, Tomahawk, Crusader,
/// Paladin, Avenger, Microwave. Fail-closed name residual (not full DieMux).
pub fn is_eject_pilot_eligible_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Exclude drones / hulks / weapons / infantry pilots themselves.
    if n.contains("drone")
        || n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("infantry")
        || n.contains("pilot")
        || n.contains("dozer")
        || n.contains("sentry")
        || n.contains("chinook")
        || n.contains("comanche")
        || n.contains("raptor")
        || n.contains("stealthfighter")
        || n.contains("aurora")
        || n.contains("jet")
        || n.contains("helicopter")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testejectvehicle"
        || n == "testejectpilotvehicle"
        || n == "goldenhumvee"
        || n == "usa_humvee"
        || n == "usa_crusader"
        || n == "usa_paladin"
        || n == "usa_tomahawk"
        || n == "usa_avenger"
        || n == "usa_microwave"
    {
        return true;
    }
    n.contains("humvee")
        || n.contains("tomahawk")
        || n.contains("tankcrusader")
        || n.contains("tankpaladin")
        || n.contains("tankavenger")
        || n.contains("tankmicrowave")
        || (n.contains("crusader") && n.contains("tank"))
        || (n.contains("paladin") && n.contains("tank"))
        || (n.contains("avenger") && n.contains("tank"))
        || (n.contains("microwave") && (n.contains("tank") || n.contains("vehicle")))
}

/// Whether residual EjectPilotDie should fire on death.
///
/// Fail-closed: eligible template, not unmanned, not under construction,
/// vehicle kind residual (not structure).
pub fn can_eject_pilot_on_death(
    is_eligible_template: bool,
    is_unmanned: bool,
    under_construction: bool,
    is_vehicle: bool,
    is_aircraft: bool,
) -> bool {
    is_eligible_template && !is_unmanned && !under_construction && is_vehicle && !is_aircraft
}

/// Rank for residual veterancy transfer (higher wins).
pub fn veterancy_rank(level: VeterancyLevel) -> u8 {
    match level {
        VeterancyLevel::Rookie => 0,
        VeterancyLevel::Veteran => 1,
        VeterancyLevel::Elite => 2,
        VeterancyLevel::Heroic => 3,
    }
}

/// Whether residual vehicle can be recrewed by a pilot.
///
/// Fail-closed: live ground vehicle, unmanned, not under construction, not aircraft.
pub fn is_recrewable_unmanned_vehicle(
    is_alive: bool,
    is_vehicle: bool,
    is_aircraft: bool,
    is_unmanned: bool,
    under_construction: bool,
    is_dozer: bool,
) -> bool {
    // Retail VeterancyCrateCollide ForbiddenKindOf = DOZER residual.
    is_alive && is_vehicle && !is_aircraft && is_unmanned && !under_construction && !is_dozer
}

/// Whether an enter command should take the pilot recrew residual path.
pub fn should_recrew_on_enter(is_pilot: bool, vehicle_recrewable: bool) -> bool {
    is_pilot && vehicle_recrewable
}

/// Merged veterancy after recrew: max(vehicle, pilot).
pub fn merged_recrew_veterancy(
    vehicle_level: VeterancyLevel,
    pilot_level: VeterancyLevel,
) -> VeterancyLevel {
    if veterancy_rank(pilot_level) >= veterancy_rank(vehicle_level) {
        pilot_level
    } else {
        vehicle_level
    }
}

/// Host residual honesty counters for USA Pilot recrew + EjectPilotDie.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostUsaPilotRegistry {
    /// Successful unmanned-vehicle recrews (pilot consumed).
    pub recrews: u32,
    /// Veterancy promotions applied onto recrewed vehicles.
    pub veterancy_transfers: u32,
    /// Successful EjectPilotDie residual pilot spawns on vehicle death.
    #[serde(default)]
    pub ejections: u32,
}

impl HostUsaPilotRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_recrew(&mut self, transferred_veterancy: bool) {
        self.recrews = self.recrews.saturating_add(1);
        if transferred_veterancy {
            self.veterancy_transfers = self.veterancy_transfers.saturating_add(1);
        }
    }

    pub fn record_ejection(&mut self) {
        self.ejections = self.ejections.saturating_add(1);
    }

    /// Residual honesty: at least one recrew completed.
    pub fn honesty_recrew_ok(&self) -> bool {
        self.recrews > 0
    }

    /// Residual honesty: recrew path with veterancy transfer observed.
    pub fn honesty_veterancy_transfer_ok(&self) -> bool {
        self.recrews > 0 && self.veterancy_transfers > 0
    }

    /// Residual honesty: at least one EjectPilotDie pilot spawn.
    pub fn honesty_eject_ok(&self) -> bool {
        self.ejections > 0
    }

    /// Combined pilot residual honesty (recrew or eject path).
    pub fn honesty_pilot_ok(&self) -> bool {
        self.honesty_recrew_ok() || self.honesty_eject_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pilot_name_matrix() {
        assert!(is_pilot_template("AmericaInfantryPilot"));
        assert!(is_pilot_template("AirF_AmericaInfantryPilot"));
        assert!(is_pilot_template("CINE_AmericaInfantryPilot"));
        assert!(is_pilot_template("TestPilot"));
        assert!(is_pilot_template("USA_Pilot"));
        assert!(!is_pilot_template("AmericaInfantryRanger"));
        assert!(!is_pilot_template("AmericaInfantryPathfinder"));
        assert!(!is_pilot_template("AmericaInfantryColonelBurton"));
        assert!(!is_pilot_template("Upgrade_AmericaPilot"));
        assert!(!is_pilot_template("GLAInfantryWorker"));
    }

    #[test]
    fn recrewable_gate() {
        assert!(is_recrewable_unmanned_vehicle(
            true, true, false, true, false, false
        ));
        assert!(!is_recrewable_unmanned_vehicle(
            true, true, false, false, false, false
        )); // manned
        assert!(!is_recrewable_unmanned_vehicle(
            true, true, true, true, false, false
        )); // aircraft
        assert!(!is_recrewable_unmanned_vehicle(
            true, true, false, true, false, true
        )); // dozer forbidden
        assert!(!is_recrewable_unmanned_vehicle(
            false, true, false, true, false, false
        )); // dead
    }

    #[test]
    fn veterancy_merge() {
        assert_eq!(
            merged_recrew_veterancy(VeterancyLevel::Rookie, VeterancyLevel::Veteran),
            VeterancyLevel::Veteran
        );
        assert_eq!(
            merged_recrew_veterancy(VeterancyLevel::Heroic, VeterancyLevel::Veteran),
            VeterancyLevel::Heroic
        );
        assert_eq!(pilot_default_veterancy(), VeterancyLevel::Veteran);
    }

    #[test]
    fn honesty_flags() {
        let mut reg = HostUsaPilotRegistry::new();
        assert!(!reg.honesty_pilot_ok());
        reg.record_recrew(true);
        assert!(reg.honesty_recrew_ok());
        assert!(reg.honesty_veterancy_transfer_ok());
        assert!(reg.honesty_pilot_ok());
        assert_eq!(reg.recrews, 1);
        assert_eq!(reg.veterancy_transfers, 1);
    }

    #[test]
    fn eject_pilot_template_matrix() {
        assert!(is_eject_pilot_eligible_template("AmericaVehicleHumvee"));
        assert!(is_eject_pilot_eligible_template("AmericaVehicleTomahawk"));
        assert!(is_eject_pilot_eligible_template("AmericaTankCrusader"));
        assert!(is_eject_pilot_eligible_template("AmericaTankPaladin"));
        assert!(is_eject_pilot_eligible_template("AmericaTankAvenger"));
        assert!(is_eject_pilot_eligible_template("AmericaTankMicrowave"));
        assert!(is_eject_pilot_eligible_template("SupW_AmericaTankCrusader"));
        assert!(is_eject_pilot_eligible_template("Lazr_AmericaTankPaladin"));
        assert!(is_eject_pilot_eligible_template("AirF_AmericaVehicleHumvee"));
        assert!(is_eject_pilot_eligible_template("TestEjectVehicle"));
        assert!(!is_eject_pilot_eligible_template("AmericaVehicleDozer"));
        assert!(!is_eject_pilot_eligible_template("AmericaVehicleScoutDrone"));
        assert!(!is_eject_pilot_eligible_template("AmericaInfantryPilot"));
        assert!(!is_eject_pilot_eligible_template("AmericaJetRaptor"));
        assert!(!is_eject_pilot_eligible_template("TestTank"));
        assert!(!is_eject_pilot_eligible_template("GLATankScorpion"));
    }

    #[test]
    fn eject_on_death_gate() {
        assert!(can_eject_pilot_on_death(true, false, false, true, false));
        assert!(!can_eject_pilot_on_death(true, true, false, true, false)); // unmanned
        assert!(!can_eject_pilot_on_death(true, false, true, true, false)); // construction
        assert!(!can_eject_pilot_on_death(false, false, false, true, false)); // ineligible
        assert!(!can_eject_pilot_on_death(true, false, false, false, false)); // not vehicle
        assert!(!can_eject_pilot_on_death(true, false, false, true, true)); // aircraft
    }

    #[test]
    fn eject_honesty_alone_is_pilot_ok() {
        let mut reg = HostUsaPilotRegistry::new();
        assert!(!reg.honesty_pilot_ok());
        reg.record_ejection();
        assert!(reg.honesty_eject_ok());
        assert!(reg.honesty_pilot_ok());
        assert_eq!(reg.ejections, 1);
    }
}
