//! Live synchronous Main owner for one ScriptEngine action walk.
use super::*;
use gamelogic::scripting::engine::{
    ScriptAiPlayerRequest, ScriptBridgeStatus, ScriptCameraRequest, ScriptDisplayRequest,
    ScriptExecutionDriver, ScriptObjectStatus, ScriptOwnerQuery, ScriptTeamStatus,
    ScriptWaterRequest,
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
    fn named_damage(&mut self, name: &str, amount: i32) -> Option<gamelogic::GameLogicResult<()>> {
        // A foreign Core registry/tracker entry must never select the receiver.
        let id = self
            .world
            .host_objects()
            .values()
            .find(|object| !object.name.is_empty() && object.name.eq_ignore_ascii_case(name))
            .map(|object| object.id);
        if let Some(id) = id {
            self.world.host_script_apply_unresistable(id, amount as f32);
        }
        Some(Ok(()))
    }

    fn water(&mut self, request: ScriptWaterRequest<'_>) -> Option<gamelogic::GameLogicResult<()>> {
        Some(self.world.apply_owned_script_water(request))
    }

    fn ai_player(
        &mut self,
        request: ScriptAiPlayerRequest<'_>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        match request {
            ScriptAiPlayerRequest::RepairNamed { player, structure } => {
                self.world.apply_owned_script_ai_repair(player, structure);
            }
            ScriptAiPlayerRequest::SelectSkillset {
                player,
                script_skillset,
            } => {
                if let Some(pid) = self.world.host_player_id_for_script_token(player) {
                    if let Some(ai) = self.world.ai_manager.ai_players.get_mut(&pid) {
                        // CPP doAffectPlayerSkillset decrements exactly once.
                        ai.select_skillset(script_skillset - 1);
                    }
                }
            }
            ScriptAiPlayerRequest::SetTeamDelay { player, seconds } => {
                if let Some(pid) = self.world.host_player_id_for_script_token(player) {
                    if let Some(ai) = self.world.ai_manager.ai_players.get_mut(&pid) {
                        ai.set_team_delay_seconds(seconds);
                    }
                }
            }
        }
        Some(Ok(()))
    }

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

    fn bridge_status(&self, name: &str) -> ScriptOwnerQuery<ScriptBridgeStatus> {
        // Resolve only this world's name/ID pair. The shared named tracker may
        // contain the same ID from a foreign world and is not an owner.
        let Some(object) = self
            .world
            .host_objects()
            .values()
            .find(|object| !object.name.is_empty() && object.name.eq_ignore_ascii_case(name))
        else {
            return ScriptOwnerQuery::Missing;
        };
        let terrain = self
            .world
            .world_services
            .terrain()
            .read()
            .expect("owned script bridge query");
        let changed = terrain.bridge_damage_states_changed();
        ScriptOwnerQuery::Present(ScriptBridgeStatus {
            broken: changed && terrain.is_bridge_broken(object.id.0),
            repaired: changed && terrain.is_bridge_repaired(object.id.0),
        })
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

#[cfg(test)]
#[path = "water_owner_tests.rs"]
mod water_owner_tests;
