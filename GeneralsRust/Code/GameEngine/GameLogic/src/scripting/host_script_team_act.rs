//! Live host drains for TEAM_MERGE_INTO_TEAM.
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

/// Remaining player-to-team relation requests awaiting owner migration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptTeamOverrideRelationRequest {
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

/// Remaining host drain for PLAYER_SET/REMOVE_OVERRIDE_RELATION_TO_TEAM.
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
    fn merge_queue_round_trips() {
        let _ = take_host_script_merge_team_requests();
        request_host_script_merge_team("USA_Src", "USA_Dest");
        assert_eq!(
            take_host_script_merge_team_requests(),
            vec![HostScriptMergeTeamRequest {
                source: "USA_Src".into(),
                dest: "USA_Dest".into(),
            }]
        );
        assert!(take_host_script_merge_team_requests().is_empty());
    }
}
