//! Live synchronous Main owner for one ScriptEngine action walk.
use super::*;
use gamelogic::scripting::engine::{
    ScriptCameraRequest, ScriptDisplayRequest, ScriptExecutionDriver, ScriptObjectStatus,
    ScriptOwnerQuery, ScriptTeamStatus,
};

pub(super) struct HostScriptExecutionDriver<'a> {
    world: &'a mut GameLogic,
}

impl<'a> HostScriptExecutionDriver<'a> {
    pub(super) fn new(world: &'a mut GameLogic) -> Self {
        Self { world }
    }

    fn status(object: &crate::game_logic::object::Object) -> ScriptObjectStatus {
        ScriptObjectStatus {
            has_ai: object.has_ai_update_interface(),
            idle: matches!(object.ai_state, AIState::Idle)
                && !object.status.moving
                && !object.status.attacking,
            effectively_dead: object.status.effectively_dead
                || object.status.destroyed
                || !object.is_alive(),
        }
    }

    fn team_owner(&self, name: &str) -> Option<Option<u32>> {
        // Query this world's existing team instances without find_team's
        // auto-creation or selecting the ambient TeamFactory.
        let teams = self.world.team_factory.lock().ok()?.get_all_teams();
        for team in teams {
            let Ok(team) = team.read() else { continue };
            if team.get_name().as_str().eq_ignore_ascii_case(name.trim()) {
                return Some(team.get_controlling_player_id());
            }
        }
        None
    }

    fn matches_team(&self, object: &crate::game_logic::object::Object, name: &str) -> bool {
        let needle = name.trim();
        if needle.is_empty() {
            return false;
        }
        // Same exact identity predicate as host_script_team_census_member_ids;
        // include dead members without joining another same-faction player.
        if !object.team_instance_name.is_empty() {
            object.team_instance_name.eq_ignore_ascii_case(needle)
        } else {
            self.world
                .default_host_team_instance_name(object.owner_player_id, object.team)
                .eq_ignore_ascii_case(needle)
        }
    }
}

impl ScriptExecutionDriver for HostScriptExecutionDriver<'_> {
    fn camera_movement_finished(&mut self) -> Option<bool> {
        Some(self.world.mission_scripts.is_camera_movement_finished())
    }

    fn camera(
        &mut self,
        request: ScriptCameraRequest<'_>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(self.world.mission_scripts.apply_camera_request(request))
    }

    fn display(
        &mut self,
        request: ScriptDisplayRequest<'_>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        // Queue on this world, at the same drain boundary as the legacy
        // callback. Do not use an engine's retained presentation owner.
        match request {
            ScriptDisplayRequest::Text(text) => {
                self.world.mission_scripts.push_message(text.into())
            }
            ScriptDisplayRequest::Cinematic {
                text,
                font,
                duration_seconds,
            } => {
                self.world.mission_scripts.push_cinematic_text(
                    text.into(),
                    font.into(),
                    duration_seconds,
                );
            }
            ScriptDisplayRequest::MilitaryCaption { text, duration_ms } => {
                self.world
                    .mission_scripts
                    .push_military_caption(text.into(), duration_ms);
            }
        }
        Some(Ok(()))
    }

    fn after_action(&mut self) -> gamelogic::GameLogicResult<()> {
        self.world.apply_script_action_requests();
        Ok(())
    }

    fn object_status(&self, id: u32) -> ScriptOwnerQuery<ScriptObjectStatus> {
        self.world
            .host_object(ObjectId(id))
            .map(Self::status)
            .map(ScriptOwnerQuery::Present)
            .unwrap_or(ScriptOwnerQuery::Missing)
    }

    fn team_status(&self, name: &str) -> ScriptOwnerQuery<ScriptTeamStatus> {
        let mut exists = self.team_owner(name).is_some();
        let mut idle = true;
        let mut dead = true;
        for object in self.world.host_objects().values() {
            if !self.matches_team(object, name) {
                continue;
            }
            exists = true;
            let status = Self::status(object);
            // CPP AIGroup.cpp:3086–3111 ignores non-AI members for idle;
            // 3151–3167 includes every member for all-dead, including an
            // empty existing team (both predicates start true).
            idle &= !status.has_ai || status.idle || status.effectively_dead;
            dead &= status.effectively_dead;
        }
        if exists {
            ScriptOwnerQuery::Present(ScriptTeamStatus {
                has_group: true,
                idle,
                dead,
            })
        } else {
            ScriptOwnerQuery::Missing
        }
    }

    fn sequential_current_player(
        &self,
        object_id: u32,
        team_name: Option<&str>,
    ) -> ScriptOwnerQuery<Option<String>> {
        let player = if let Some(object) = self.world.host_object(ObjectId(object_id)) {
            object.owner_player_id
        } else if let Some(name) = team_name {
            if let Some(owner) = self.team_owner(name) {
                owner
            } else {
                self.world
                    .host_objects()
                    .values()
                    .find(|object| self.matches_team(object, name))
                    .and_then(|object| object.owner_player_id)
            }
        } else {
            None
        };
        // This Main AI manager holds the actual skirmish opponents; is_local
        // alone cannot distinguish a remote human from an AI owner.
        ScriptOwnerQuery::Present(
            player
                .filter(|id| self.world.ai_manager.ai_players.contains_key(id))
                .and_then(|id| self.world.players.get(&id))
                .map(|player| player.name.clone()),
        )
    }
}
