//! Live synchronous Main owner for one ScriptEngine action walk.
use super::*;
use gamelogic::scripting::engine::{
    ScriptAiPlayerRequest, ScriptBridgeStatus, ScriptCameraRequest, ScriptDisplayRequest,
    ScriptExecutionDriver, ScriptNamedCommand, ScriptObjectStatus, ScriptOwnerQuery,
    ScriptPlayerEventSource, ScriptTeamStatus, ScriptWaterRequest,
};
use std::sync::{Arc, RwLock};

pub(super) struct HostScriptExecutionDriver<'a> {
    world: &'a mut GameLogic,
}

impl<'a> HostScriptExecutionDriver<'a> {
    pub(super) fn new(world: &'a mut GameLogic) -> Self {
        Self { world }
    }

    fn player_script_name(player: &Player) -> &str {
        if player.map_side.map_player_name.is_empty() {
            &player.name
        } else {
            &player.map_side.map_player_name
        }
    }

    fn named_player_id(&self, name: &str) -> Option<u32> {
        // CPP initFromDict/ScriptEngine5810 use the authored name key,
        // not a display name or an invented faction alias.
        self.world
            .players
            .values()
            .find(|player| Self::player_script_name(player) == name)
            .map(|player| player.id)
    }

    fn requires_prepared_player_bindings(&self) -> bool {
        // Match the map loader's prepared-side modes. Their side indices and
        // authored player names are not Main host IDs/display labels. Until
        // admission owns that mapping, retain the existing condition adapter.
        matches!(
            self.world.game_mode,
            GameMode::Skirmish
                | GameMode::Multiplayer
                | GameMode::Lan
                | GameMode::Internet
                | GameMode::Replay
        )
    }

    fn script_player_id(&self, token: &str, current_player: Option<&str>) -> Option<u32> {
        use gamelogic::scripting::core::{
            LOCAL_PLAYER, THE_PLAYER, THIS_PLAYER, THIS_PLAYER_ENEMY,
        };
        match token {
            THIS_PLAYER => self.named_player_id(current_player?),
            LOCAL_PLAYER => self
                .world
                .players
                .values()
                .find(|p| p.is_local)
                .map(|p| p.id),
            THIS_PLAYER_ENEMY => {
                let current = self.named_player_id(current_player?)?;
                self.world
                    .ai_manager
                    .ai_players
                    .get(&current)
                    .and_then(|ai| ai.enemy_player_id)
                    .filter(|id| self.world.players.contains_key(id))
                    .or_else(|| {
                        self.world
                            .players
                            .values()
                            .filter(|p| {
                                p.is_human
                                    && !(gamelogic::scripting::core::is_generals_challenge_campaign(
                                    ) && Self::player_script_name(p) == THE_PLAYER)
                            })
                            .min_by_key(|p| p.id)
                            .map(|p| p.id)
                    })
            }
            // Retain the existing Challenge classification. Campaign metadata
            // ownership is separate from this query's player/geometry owner.
            THE_PLAYER if gamelogic::scripting::core::is_generals_challenge_campaign() => self
                .world
                .players
                .values()
                .find(|p| p.is_local)
                .map(|p| p.id),
            _ => self.named_player_id(token),
        }
    }

    fn qualified_area(&self, area: &str, current_player: Option<&str>) -> Option<String> {
        let (enemy, perimeter) = match area {
            "[Skirmish]MyInnerPerimeter" => (false, "InnerPerimeter"),
            "[Skirmish]MyOuterPerimeter" => (false, "OuterPerimeter"),
            "[Skirmish]EnemyInnerPerimeter" => (true, "InnerPerimeter"),
            "[Skirmish]EnemyOuterPerimeter" => (true, "OuterPerimeter"),
            _ => return Some(area.into()),
        };
        let current = current_player.and_then(|name| self.named_player_id(name));
        let player = if enemy {
            current
                .and_then(|id| self.world.ai_manager.ai_players.get(&id))
                .and_then(|ai| ai.enemy_player_id)
        } else {
            Some(current?)
        };
        let index = player
            .and_then(|id| self.world.players.get(&id))
            .map(|p| p.start_position + 1);
        Some(format!("{perimeter}{}", index.unwrap_or(-1)))
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

    fn owned_team(&self, id: u32) -> Option<Arc<RwLock<gamelogic::team::Team>>> {
        self.world
            .team_factory
            .lock()
            .expect("owned script team factory")
            .find_team_by_id(id)
    }
}

impl ScriptExecutionDriver for HostScriptExecutionDriver<'_> {
    fn skirmish_player_can_build_any(
        &self,
        player: &str,
        object_types: &[String],
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<bool> {
        if self.requires_prepared_player_bindings() {
            return ScriptOwnerQuery::Unavailable;
        }
        let Some(player) = self
            .script_player_id(player, current_player)
            .and_then(|id| self.world.players.get(&id))
        else {
            return ScriptOwnerQuery::Missing;
        };
        ScriptOwnerQuery::Present(
            object_types
                .iter()
                .any(|name| self.world.script_player_can_build_template(player, name)),
        )
    }

    fn player_event_source(
        &self,
        player: &str,
        source: Option<&str>,
        current_player: Option<&str>,
        this_object: Option<gamelogic::common::ObjectID>,
    ) -> ScriptOwnerQuery<ScriptPlayerEventSource> {
        // Prepared map-side indices still require explicit admission (hq-fxwkm).
        // Ordinary Main notifications use the controlling host player ID.
        if self.requires_prepared_player_bindings() {
            return ScriptOwnerQuery::Unavailable;
        }
        let Some(player_index) = self.script_player_id(player, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
        let source_object = match source {
            None => gamelogic::common::INVALID_ID,
            Some(gamelogic::scripting::core::THIS_OBJECT) => {
                let Some(id) =
                    this_object.filter(|id| self.world.host_object(ObjectId(*id)).is_some())
                else {
                    return ScriptOwnerQuery::Missing;
                };
                id
            }
            Some(name) => {
                let Some(id) = self.world.host_objects().values().find_map(|object| {
                    (!name.is_empty() && object.name == name).then_some(object.id.0)
                }) else {
                    return ScriptOwnerQuery::Missing;
                };
                id
            }
        };
        ScriptOwnerQuery::Present(ScriptPlayerEventSource {
            player_index: player_index as usize,
            source_object,
        })
    }

    fn team_guard(
        &mut self,
        team: &str,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        // CPP5936 returns the exact default Team pointer in Challenge mode.
        // That identity is not yet admitted by Main (hq-dryhs/hq-9bqdm);
        // retain the existing adapter for this capability rather than invent
        // a team from a display name or choose an unrelated same-name instance.
        if team == gamelogic::scripting::core::TEAM_THE_PLAYER
            && gamelogic::scripting::core::is_generals_challenge_campaign()
        {
            return None;
        }
        Some(
            self.world
                .apply_owned_team_guard(team, calling_team, condition_team),
        )
    }

    fn team_attitude(
        &mut self,
        team: &str,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
        mood: i32,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        if team == gamelogic::scripting::core::TEAM_THE_PLAYER
            && gamelogic::scripting::core::is_generals_challenge_campaign()
        {
            return None;
        }
        Some(
            self.world
                .apply_owned_team_attitude(team, calling_team, condition_team, mood),
        )
    }

    fn team_sequential(
        &mut self,
        team: &str,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
        script: Option<(gamelogic::scripting::core::Script, i32)>,
        engine: &gamelogic::scripting::engine::ScriptEngine,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        if team == gamelogic::scripting::core::TEAM_THE_PLAYER
            && gamelogic::scripting::core::is_generals_challenge_campaign()
        {
            return None;
        }
        Some(self.world.apply_owned_team_sequential(
            team,
            calling_team,
            condition_team,
            script,
            engine,
        ))
    }

    fn named_command(
        &mut self,
        request: ScriptNamedCommand<'_>,
        this_object: Option<gamelogic::common::ObjectID>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.world
            .apply_owned_named_script_command(request, this_object.map(ObjectId));
        // A missing object is the original no-op, never permission to search
        // another session's registry or enqueue an ambient command.
        Some(Ok(()))
    }

    fn skirmish_player_exists(
        &self,
        player: &str,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<()> {
        if self.requires_prepared_player_bindings() {
            return ScriptOwnerQuery::Unavailable;
        }
        if self.script_player_id(player, current_player).is_some() {
            ScriptOwnerQuery::Present(())
        } else {
            ScriptOwnerQuery::Missing
        }
    }

    fn tech_building_within_distance(
        &self,
        player: &str,
        distance: f32,
        area: &str,
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<bool> {
        use gamelogic::common::Relationship;
        if self.requires_prepared_player_bindings() {
            return ScriptOwnerQuery::Unavailable;
        }
        let Some(player) = self
            .script_player_id(player, current_player)
            .and_then(|id| self.world.players.get(&id))
        else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(area) = self.qualified_area(area, current_player) else {
            return ScriptOwnerQuery::Missing;
        };
        let Some(trigger) = self
            .world
            .host_trigger_world
            .lock()
            .expect("owned script trigger query")
            .trigger_area_by_name(&area)
        else {
            return ScriptOwnerQuery::Missing;
        };
        // Avoid get_center_point's ambient terrain-height lookup. FROM_CENTER_2D
        // uses only the polygon bounds; get_radius preserves CPP's formula.
        let bounds = trigger.get_bounds();
        let center_x = (bounds.lo.x + bounds.hi.x) as f32 / 2.0;
        let center_z = (bounds.lo.y + bounds.hi.y) as f32 / 2.0;
        let radius = trigger.get_radius() + distance;
        let radius_sq = radius * radius;
        let found = self.world.host_objects().values().any(|object| {
            let position = object.get_position();
            if !object.is_kind_of(KindOf::TechBuilding)
                || object.owner_player_id == Some(player.id)
                || position.x < self.world.world_min.x
                || position.x > self.world.world_max.x
                || position.z < self.world.world_min.z
                || position.z > self.world.world_max.z
            {
                return false;
            }
            let team = if object.team_instance_name.is_empty() {
                self.world
                    .default_host_team_instance_name(object.owner_player_id, object.team)
            } else {
                object.team_instance_name.clone()
            };
            let relationship = player.team_relationship_override(&team).unwrap_or_else(|| {
                let Some(target) = object
                    .owner_player_id
                    .and_then(|id| self.world.players.get(&id))
                else {
                    return Relationship::Neutral;
                };
                // CPP Player542 reads relationships even for defeated players.
                player.map_relationship(target.id).unwrap_or_else(|| {
                    if player.alliance_team >= 0 && player.alliance_team == target.alliance_team {
                        Relationship::Allies
                    } else if player.alliance_team >= 0 && target.alliance_team >= 0 {
                        Relationship::Enemies
                    } else {
                        Relationship::Neutral
                    }
                })
            });
            if relationship == Relationship::Allies {
                return false;
            }
            let dx = position.x - center_x;
            let dz = position.z - center_z;
            dx * dx + dz * dz < radius_sq
        });
        ScriptOwnerQuery::Present(found)
    }

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

    fn team_instances(&self, name: &str) -> ScriptOwnerQuery<Vec<u32>> {
        let factory = self
            .world
            .team_factory
            .lock()
            .expect("owned script team instances");
        if factory.find_team_prototype(name).is_none() {
            return ScriptOwnerQuery::Missing;
        }
        ScriptOwnerQuery::Present(
            factory
                .find_team_instances(name)
                .iter()
                .map(|team| team.read().expect("owned script team instance").get_id())
                .collect(),
        )
    }

    fn team_name(&self, id: u32) -> ScriptOwnerQuery<String> {
        let Some(team) = self.owned_team(id) else {
            return ScriptOwnerQuery::Missing;
        };
        let name = team
            .read()
            .expect("owned script team name")
            .get_name()
            .to_string();
        ScriptOwnerQuery::Present(name)
    }

    fn team_current_player(&self, id: u32) -> ScriptOwnerQuery<Option<String>> {
        let Some(team) = self.owned_team(id) else {
            return ScriptOwnerQuery::Missing;
        };
        let owner = team
            .read()
            .expect("owned script team controller")
            .get_controlling_player_id();
        ScriptOwnerQuery::Present(
            owner
                .and_then(|id| self.world.players.get(&id))
                .map(|player| Self::player_script_name(player).to_string()),
        )
    }

    fn team_status(&self, id: u32) -> ScriptOwnerQuery<ScriptTeamStatus> {
        let Some(team) = self.owned_team(id) else {
            return ScriptOwnerQuery::Missing;
        };
        let members = team
            .read()
            .expect("owned script team roster")
            .get_members()
            .to_vec();
        let mut idle = true;
        let mut dead = true;
        for id in members {
            let Some(object) = self.world.host_object(ObjectId(id)) else {
                continue;
            };
            let status = Self::status(object);
            // CPP AIGroup.cpp3086/3151: non-AI members do not affect idle;
            // every admitted live member affects all-dead. Empty groups are both.
            idle &= !status.has_ai || status.idle || status.effectively_dead;
            dead &= status.effectively_dead;
        }
        ScriptOwnerQuery::Present(ScriptTeamStatus {
            has_group: true,
            idle,
            dead,
        })
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
        team_id: Option<u32>,
    ) -> ScriptOwnerQuery<Option<String>> {
        let player = if let Some(object) = self.world.host_object(ObjectId(object_id)) {
            object.owner_player_id
        } else if let Some(id) = team_id {
            let Some(team) = self.owned_team(id) else {
                return ScriptOwnerQuery::Missing;
            };
            let owner = team
                .read()
                .expect("owned sequential team controller")
                .get_controlling_player_id();
            owner
        } else {
            None
        };
        // CPP7888: object controller precedes team controller; only SkirmishAI
        // becomes the sequential current player. Human/absent owners are final.
        ScriptOwnerQuery::Present(
            player
                .filter(|id| self.world.ai_manager.ai_players.contains_key(id))
                .and_then(|id| self.world.players.get(&id))
                .map(|player| Self::player_script_name(player).to_string()),
        )
    }
}

#[cfg(test)]
#[path = "water_owner_tests.rs"]
mod water_owner_tests;
