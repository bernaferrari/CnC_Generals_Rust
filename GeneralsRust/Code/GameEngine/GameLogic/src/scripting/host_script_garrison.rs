//! Live host drains for the enter / garrison / exit script-action family.
//!
//! Kept out of `executor/mod.rs` so leftover action files can queue without
//! colliding with other executor edits.
//!
//! C++ `ScriptActions::doNamedEnterNamed` / `doTeamEnterNamed` /
//! `doNamedExitAll` / `doTeamExitAll` / `doTeamGarrisonSpecificBuilding` /
//! `doExitSpecificBuilding` / `doTeamGarrisonNearestBuilding` /
//! `doTeamExitAllBuildings` / `doUnitGarrisonSpecificBuilding` /
//! `doUnitGarrisonNearestBuilding` / `doUnitExitBuilding` /
//! `doPlayerGarrisonAllBuildings` / `doPlayerExitAllBuildings`.

use std::cell::RefCell;

/// Live host drain: enter / garrison / evacuate / exit.
/// Leftover `OBJECT_REGISTRY` is empty on the player path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptGarrisonEnterExitRequest {
    NamedEnter { unit: String, dest: String },
    TeamEnter { team: String, dest: String },
    NamedExitAll { unit: String },
    TeamExitAll { team: String },
    TeamGarrisonSpecific { team: String, building: String },
    ExitSpecificBuilding { building: String },
    TeamGarrisonNearest { team: String },
    TeamExitAllBuildings { team: String },
    NamedGarrisonSpecific { unit: String, building: String },
    NamedGarrisonNearest { unit: String },
    NamedExitBuilding { unit: String },
    PlayerGarrisonAll { player: String },
    PlayerExitAll { player: String },
}

thread_local! {
    static HOST_SCRIPT_GARRISON_ENTER_EXIT_REQUESTS:
        RefCell<Vec<HostScriptGarrisonEnterExitRequest>> = RefCell::new(Vec::new());
}

/// Live host drain: leftover-queue `aiEnter` / `aiEvacuate` / `aiExit`.
/// Leftover crate objects are empty on the player path.
pub fn request_host_script_garrison_enter(req: HostScriptGarrisonEnterExitRequest) {
    HOST_SCRIPT_GARRISON_ENTER_EXIT_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_garrison_enter_requests() -> Vec<HostScriptGarrisonEnterExitRequest> {
    HOST_SCRIPT_GARRISON_ENTER_EXIT_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garrison_enter_exit_queues_round_trip() {
        let _ = take_host_script_garrison_enter_requests();
        request_host_script_garrison_enter(HostScriptGarrisonEnterExitRequest::NamedEnter {
            unit: "Ranger01".into(),
            dest: "Humvee01".into(),
        });
        request_host_script_garrison_enter(
            HostScriptGarrisonEnterExitRequest::TeamGarrisonNearest {
                team: "USA_Infantry".into(),
            },
        );
        request_host_script_garrison_enter(HostScriptGarrisonEnterExitRequest::PlayerExitAll {
            player: "PlyrAmerica".into(),
        });
        assert_eq!(
            take_host_script_garrison_enter_requests(),
            vec![
                HostScriptGarrisonEnterExitRequest::NamedEnter {
                    unit: "Ranger01".into(),
                    dest: "Humvee01".into(),
                },
                HostScriptGarrisonEnterExitRequest::TeamGarrisonNearest {
                    team: "USA_Infantry".into(),
                },
                HostScriptGarrisonEnterExitRequest::PlayerExitAll {
                    player: "PlyrAmerica".into(),
                },
            ]
        );
        assert!(take_host_script_garrison_enter_requests().is_empty());
    }
}
