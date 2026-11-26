//! ScriptActions.cpp:1082–1087,2284–2310: owned AI build/recruit admission.
use super::*;

#[path = "script_ordinary_supply_build.rs"]
mod script_ordinary_supply_build;

pub(super) struct OrdinarySupplyBuild {
    warehouse: ObjectId,
    pub(super) toward_enemy: bool,
}

fn ordinary_priority_build_row(
    building: &str,
    position: Vec3,
    angle: f32,
) -> crate::ai::AIBuildingInfo {
    use gamelogic::build_list_info::BuildListInfo;
    use glam::Vec3 as Coord3D;

    let mut entry = BuildListInfo::new();
    entry.set_template_name(building.into());
    entry.set_location(Coord3D::new(position.x, position.z, position.y));
    entry.set_angle(angle);
    entry.set_num_rebuilds(1);
    entry.mark_priority_build();
    crate::ai::AIBuildingInfo::from_campaign_build_list(&entry)
}

impl GameLogic {
    pub(super) fn apply_owned_script_team_build(
        &mut self,
        team: &str,
        recruit_radius: Option<f32>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) {
        self.apply_owned_script_team_build_with_effects(
            team,
            recruit_radius,
            interpreter,
            &mut Default::default(),
        )
    }

    pub(super) fn apply_owned_script_team_build_with_effects(
        &mut self,
        team: &str,
        recruit_radius: Option<f32>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // C++ requires the prototype controller; neither a live team nor the
        // script caller substitutes for a missing prototype/controller.
        let controller = self
            .team_factory
            .with_read_without_flushing(|factory| factory.admitted_prototype_controller(team))
            .unwrap_or_else(|error| error.into_inner())
            .flatten();
        let Some(controller) = controller else {
            return;
        };
        if let Some(radius) = recruit_radius {
            self.recruit_owned_specific_ai_team_with_effects(
                controller,
                team,
                radius,
                interpreter,
                ui,
            );
        } else {
            self.build_owned_specific_ai_team_with_effects(controller, team, true, interpreter, ui);
        }
    }

    pub(super) fn apply_owned_script_supply_build(
        &mut self,
        player: &str,
        building: &str,
        cash: i32,
    ) {
        if let Some(id) = crate::ai::AIManager::resolve_player_id(self, player) {
            self.query_owned_ai_enemy(id);
        }
        let mut manager = std::mem::take(&mut self.ai_manager);
        manager.build_by_supplies_for_token(self, player, cash, building);
        self.ai_manager = manager;
    }
}

impl GameLogic {
    pub(super) fn apply_owned_script_build_nearest(
        &mut self,
        player: &str,
        building: &str,
        team: &str,
        calling: Option<u32>,
        condition: Option<u32>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        let Some(player) = self.host_player_id_for_script_token(player) else {
            return Ok(());
        };
        let Some(ai) = self.ai_manager.ai_players.get(&player) else {
            return Ok(());
        };
        let ordinary = ai.ordinary_timing.is_some();
        let Some((_, members)) = self.resolve_owned_script_team(team, calling, condition)? else {
            return Ok(());
        };
        if ordinary {
            // Team.cpp:2206 uses the intrusive list head, even if it is
            // effectively dead, rather than searching for another member.
            let Some(position) = members
                .first()
                .and_then(|&id| self.host_object(ObjectId(id)))
                .map(|object| object.get_position())
            else {
                return Ok(());
            };
            if let Some((position, angle)) =
                self.ordinary_nearest_team_build_site(player, building, position, effects)
            {
                self.ai_manager
                    .ai_players
                    .get_mut(&player)
                    .unwrap()
                    .building_queue
                    .insert(0, ordinary_priority_build_row(building, position, angle));
            }
            return Ok(());
        }

        let Some(position) = members.into_iter().find_map(|id| {
            self.host_object(ObjectId(id))
                .filter(|o| o.is_alive())
                .map(|o| o.get_position())
        }) else {
            return Ok(());
        };
        if !building.trim().is_empty() {
            self.ai_manager
                .ai_players
                .get_mut(&player)
                .unwrap()
                .add_building(building, position, 2);
        }
        Ok(())
    }

    pub(super) fn apply_owned_script_base_defense(
        &mut self,
        player: &str,
        structure: Option<&str>,
        flank: bool,
    ) {
        let Some(player) = self.host_player_id_for_script_token(player) else {
            return;
        };
        // AIPlayer.cpp:2234–2249 handles these commands only in the subclass.
        if self
            .ai_manager
            .ai_players
            .get(&player)
            .is_some_and(|ai| ai.ordinary_timing.is_some())
        {
            return;
        }
        // Placement only observes owned state and admits an AI build-list row;
        // it executes no factory/script callback while the controller is borrowed.
        let Some(mut ai) = self.ai_manager.ai_players.remove(&player) else {
            return;
        };
        if let Some(structure) = structure {
            ai.build_script_base_defense_structure(Some(self), structure, flank);
        } else {
            ai.build_script_base_defense(Some(self), flank);
        }
        self.ai_manager.ai_players.insert(player, ai);
    }
}

impl GameLogic {
    pub(super) fn apply_owned_team_wander(
        &mut self,
        team: &str,
        path: Option<&str>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> generals_sim::GameLogicResult<()> {
        let Some((_, members)) = self.resolve_owned_script_team(team, calling, condition)? else {
            return Ok(());
        };
        for id in members {
            let id = ObjectId(id);
            let Some(position) = self
                .host_object(id)
                .filter(|object| object.has_ai_update_interface())
                .map(|object| object.get_position())
            else {
                continue;
            };
            if let Some(label) = path {
                // C++ chooses the closest waypoint before switching this
                // member's locomotor; a missing path aborts the remaining team.
                let Some(waypoints) = self.host_wander_waypoint_path_from(label, position) else {
                    return Ok(());
                };
                let _ = self.apply_unit_locomotor_set(id, "wander");
                self.host_wander_issue_path(id, &waypoints);
            } else {
                let _ = self.apply_unit_locomotor_set(id, "wander");
                self.host_wander_in_place(id, position);
            }
        }
        Ok(())
    }

    pub(super) fn apply_owned_team_panic(
        &mut self,
        team: &str,
        path: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> generals_sim::GameLogicResult<()> {
        let Some((_, members)) = self.resolve_owned_script_team(team, calling, condition)? else {
            return Ok(());
        };
        for id in members {
            let id = ObjectId(id);
            let Some(position) = self
                .host_object(id)
                .filter(|object| object.has_ai_update_interface())
                .map(|object| object.get_position())
            else {
                continue;
            };
            // CPP2454: one closest waypoint per member; missing aborts the
            // whole remaining loop before any locomotor switch on that member.
            let Some((waypoint, goal, links, extra)) =
                self.host_panic_waypoint_from(path, position)
            else {
                return Ok(());
            };
            self.host_start_panic(id, waypoint, goal, links, extra, "panic");
        }
        Ok(())
    }
}
