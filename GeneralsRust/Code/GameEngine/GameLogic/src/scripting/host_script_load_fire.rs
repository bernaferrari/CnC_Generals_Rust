//! Live host drain for NAMED_FIRE_WEAPON_FOLLOWING_WAYPOINT_PATH.
//!
//! Kept out of `executor/mod.rs` / `host_script_team_act.rs` so leftover
//! action files can queue without colliding with other wave-29 executor edits.

use std::cell::RefCell;

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
    static HOST_SCRIPT_NAMED_FIRE_WEAPON_PATH_REQUESTS:
        RefCell<Vec<HostScriptNamedFireWeaponPathRequest>> = RefCell::new(Vec::new());
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
    fn named_fire_queue_round_trips() {
        let _ = take_host_script_named_fire_weapon_path_requests();
        request_host_script_named_fire_weapon_path("NamedScud", "CruisePath");
        assert_eq!(
            take_host_script_named_fire_weapon_path_requests(),
            vec![HostScriptNamedFireWeaponPathRequest {
                unit: "NamedScud".into(),
                waypoint: "CruisePath".into(),
            }]
        );
        assert!(take_host_script_named_fire_weapon_path_requests().is_empty());
    }
}
