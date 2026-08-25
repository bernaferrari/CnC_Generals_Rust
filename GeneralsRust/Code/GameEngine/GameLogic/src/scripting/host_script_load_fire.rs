//! Live host drains for TEAM_LOAD_TRANSPORTS and
//! NAMED_FIRE_WEAPON_FOLLOWING_WAYPOINT_PATH.
//!
//! Kept out of `executor/mod.rs` / `host_script_team_act.rs` so leftover
//! action files can queue without colliding with other wave-29 executor edits.

use std::cell::RefCell;

/// Live host drain: TEAM_LOAD_TRANSPORTS.
/// C++ `ScriptActions::doLoadAllTransports` (`PartitionSolver` PREFER_FAST
/// then `chooseLocomotorSet(NORMAL)` + `aiEnter(..., CMD_FROM_SCRIPT)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostScriptLoadTransportsRequest {
    pub team: String,
}

/// Live host drain: NAMED_FIRE_WEAPON_FOLLOWING_WAYPOINT_PATH.
/// C++ `ScriptActions::doNamedFireWeaponFollowingWaypointPath`
/// (`findWaypointFollowingCapableWeapon` + `forceFireWeapon` then projectile
/// `leaveGroup` + `chooseLocomotorSet(NORMAL)` + `aiFollowWaypointPath`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostScriptNamedFireWeaponPathRequest {
    pub unit: String,
    pub waypoint: String,
}

thread_local! {
    static HOST_SCRIPT_LOAD_TRANSPORTS_REQUESTS:
        RefCell<Vec<HostScriptLoadTransportsRequest>> = RefCell::new(Vec::new());
    static HOST_SCRIPT_NAMED_FIRE_WEAPON_PATH_REQUESTS:
        RefCell<Vec<HostScriptNamedFireWeaponPathRequest>> = RefCell::new(Vec::new());
}

/// Live host drain: leftover BinPartitionSolver + enter on live transports.
/// Leftover `TheGameLogic::find_object_by_id` is empty on the player path.
pub fn request_host_script_load_transports(team: &str) {
    HOST_SCRIPT_LOAD_TRANSPORTS_REQUESTS.with(|q| {
        q.borrow_mut().push(HostScriptLoadTransportsRequest {
            team: team.to_string(),
        });
    });
}

pub fn take_host_script_load_transports_requests() -> Vec<HostScriptLoadTransportsRequest> {
    HOST_SCRIPT_LOAD_TRANSPORTS_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: leftover forceFire + follow waypoint path on live projectile.
/// Leftover crate `force_fire_weapon` cannot spawn a live host projectile.
pub fn request_host_script_named_fire_weapon_path(unit: &str, waypoint: &str) {
    HOST_SCRIPT_NAMED_FIRE_WEAPON_PATH_REQUESTS.with(|q| {
        q.borrow_mut().push(HostScriptNamedFireWeaponPathRequest {
            unit: unit.to_string(),
            waypoint: waypoint.to_string(),
        });
    });
}

pub fn take_host_script_named_fire_weapon_path_requests()
-> Vec<HostScriptNamedFireWeaponPathRequest> {
    HOST_SCRIPT_NAMED_FIRE_WEAPON_PATH_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_transports_and_named_fire_queues_round_trip() {
        let _ = take_host_script_load_transports_requests();
        let _ = take_host_script_named_fire_weapon_path_requests();
        request_host_script_load_transports("USA_Convoy");
        request_host_script_named_fire_weapon_path("NamedScud", "CruisePath");
        assert_eq!(
            take_host_script_load_transports_requests(),
            vec![HostScriptLoadTransportsRequest {
                team: "USA_Convoy".into(),
            }]
        );
        assert_eq!(
            take_host_script_named_fire_weapon_path_requests(),
            vec![HostScriptNamedFireWeaponPathRequest {
                unit: "NamedScud".into(),
                waypoint: "CruisePath".into(),
            }]
        );
        assert!(take_host_script_load_transports_requests().is_empty());
        assert!(take_host_script_named_fire_weapon_path_requests().is_empty());
    }
}
