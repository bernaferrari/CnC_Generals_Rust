//! Live host drains for TEAM_MERGE_INTO_TEAM and
//! TEAM_CAPTURE_NEAREST_UNOWNED_FACTION_UNIT.
//!
//! Kept out of `executor/mod.rs` so leftover action files can queue without
//! colliding with other wave-29 executor edits.

use std::cell::RefCell;

/// Live host drain: TEAM_MERGE_INTO_TEAM.
/// C++ `ScriptActions::doMergeTeamIntoTeam` (`setTeam` + `updateTeamAndPlayerStuff`
/// + `deleteTeam` + dest `setActive`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostScriptMergeTeamRequest {
    pub source: String,
    pub dest: String,
}

/// Live host drain: TEAM_CAPTURE_NEAREST_UNOWNED_FACTION_UNIT.
/// C++ `ScriptActions::doTeamCaptureNearestUnownedFactionUnit`
/// (`getClosestObject` unmanned + enemies/neutral + on-map, then `groupEnter`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostScriptCaptureNearestUnownedRequest {
    pub team: String,
}

/// Live host drain: TEAM_SET/REMOVE_OVERRIDE_RELATION_TO_TEAM/PLAYER
/// and TEAM_REMOVE_ALL_OVERRIDE_RELATIONS, plus
/// PLAYER_SET/REMOVE_OVERRIDE_RELATION_TO_TEAM.
/// C++ `doTeamSetOverrideRelationToTeam` / `ToPlayer` / removers write
/// `Team::m_teamRelations` / `m_playerRelations`. `doPlayerSetOverrideRelationToTeam`
/// writes `Player::m_teamRelations`. Leftover maps stay leftover-right; live
/// combat and `VictoryConditions::areAllies` read host Player maps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptTeamOverrideRelationRequest {
    SetTeam {
        source: String,
        dest_team: String,
        relationship: crate::common::Relationship,
    },
    RemoveTeam {
        source: String,
        dest_team: String,
    },
    SetPlayer {
        source: String,
        dest_player: String,
        relationship: crate::common::Relationship,
    },
    RemovePlayer {
        source: String,
        dest_player: String,
    },
    RemoveAll {
        source: String,
    },
    SetPlayerToTeam {
        source_player: String,
        dest_team: String,
        relationship: crate::common::Relationship,
    },
    RemovePlayerToTeam {
        source_player: String,
        dest_team: String,
    },
}

thread_local! {
    static HOST_SCRIPT_MERGE_TEAM_REQUESTS: RefCell<Vec<HostScriptMergeTeamRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_CAPTURE_NEAREST_UNOWNED_REQUESTS:
        RefCell<Vec<HostScriptCaptureNearestUnownedRequest>> = RefCell::new(Vec::new());
    static HOST_SCRIPT_TEAM_OVERRIDE_RELATION_REQUESTS:
        RefCell<Vec<HostScriptTeamOverrideRelationRequest>> = RefCell::new(Vec::new());
}

/// Live host drain: TEAM_MERGE_INTO_TEAM rewrites live `team_instance_name`.
pub fn request_host_script_merge_team(source: &str, dest: &str) {
    HOST_SCRIPT_MERGE_TEAM_REQUESTS.with(|q| {
        q.borrow_mut().push(HostScriptMergeTeamRequest {
            source: source.to_string(),
            dest: dest.to_string(),
        });
    });
}

pub fn take_host_script_merge_team_requests() -> Vec<HostScriptMergeTeamRequest> {
    HOST_SCRIPT_MERGE_TEAM_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: TEAM_CAPTURE_NEAREST_UNOWNED_FACTION_UNIT.
/// Leftover partition / leftover crate objects are empty on the player path.
pub fn request_host_script_capture_nearest_unowned(team: &str) {
    HOST_SCRIPT_CAPTURE_NEAREST_UNOWNED_REQUESTS.with(|q| {
        q.borrow_mut().push(HostScriptCaptureNearestUnownedRequest {
            team: team.to_string(),
        });
    });
}

pub fn take_host_script_capture_nearest_unowned_requests()
-> Vec<HostScriptCaptureNearestUnownedRequest> {
    HOST_SCRIPT_CAPTURE_NEAREST_UNOWNED_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: TEAM_SET/REMOVE_OVERRIDE_RELATION_* leftover Team maps.
pub fn request_host_team_override_relation(req: HostScriptTeamOverrideRelationRequest) {
    HOST_SCRIPT_TEAM_OVERRIDE_RELATION_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_team_override_relation_requests() -> Vec<HostScriptTeamOverrideRelationRequest> {
    HOST_SCRIPT_TEAM_OVERRIDE_RELATION_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_and_capture_queues_round_trip() {
        let _ = take_host_script_merge_team_requests();
        let _ = take_host_script_capture_nearest_unowned_requests();
        request_host_script_merge_team("USA_Src", "USA_Dest");
        request_host_script_capture_nearest_unowned("USA_Hijack");
        assert_eq!(
            take_host_script_merge_team_requests(),
            vec![HostScriptMergeTeamRequest {
                source: "USA_Src".into(),
                dest: "USA_Dest".into(),
            }]
        );
        assert_eq!(
            take_host_script_capture_nearest_unowned_requests(),
            vec![HostScriptCaptureNearestUnownedRequest {
                team: "USA_Hijack".into(),
            }]
        );
        assert!(take_host_script_merge_team_requests().is_empty());
        assert!(take_host_script_capture_nearest_unowned_requests().is_empty());
    }
}
