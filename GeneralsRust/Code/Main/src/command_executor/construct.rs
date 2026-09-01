//! Dozer/build/resume/cancel construction.
#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
use crate::command_system::{
    CommandResult, CommandType, DropTarget, GameCommand, GuardTarget, PowerTarget,
    SpecialPowerType, WeaponSlot, WeaponTarget,
};
use crate::game_logic::game_logic::AudioEventRequest;
use crate::game_logic::{
    AIState, GameLogic, KindOf, ObjectId, ObjectType, PendingSpecialAbility, Resources, Team,
    radar_notifications::RadarKind,
};
use crate::localization;
use crate::ui::audio::translate_audio_event;
use gamelogic::common::AsciiString;
use gamelogic::common::types::Coord3D as LogicCoord3D;
use gamelogic::system::beacon_manager::get_beacon_manager;
use gamelogic::system::game_logic::current_frame;
use glam::Vec3;
use log::{debug, warn};
use std::collections::{HashMap, HashSet};

impl<'a> CommandExecutor<'a> {
    // === Construction Commands ===

    pub(super) fn execute_build(
        &mut self,
        units: &[ObjectId],
        template_name: &str,
        location: Vec3,
        orientation: f32,
    ) -> CommandResult {
        if !self.validate_build_location(location) {
            return CommandResult::InvalidLocation;
        }

        let (base_cost, is_structure) = match self.game_logic.get_templates().get(template_name) {
            Some(t) => (t.build_cost, t.is_kind_of(KindOf::Structure)),
            None => return CommandResult::InvalidCommand,
        };
        let build_cost = self.calc_cost_to_build(template_name, base_cost);

        if !is_structure {
            return CommandResult::InvalidCommand;
        }

        for &unit_id in units {
            let team = match self.game_logic.host_object(unit_id) {
                Some(unit)
                    if unit.can_construct()
                        && unit.owner_player_id == Some(self.current_player_id) =>
                {
                    unit.team
                }
                Some(_) => continue,
                None => continue,
            };

            // C++ BuildAssistant CLEAR_PATH residual before charging resources.
            if !self.game_logic.is_location_legal_to_build_for_builder(
                team,
                location,
                template_name,
                Some(unit_id),
            ) {
                return CommandResult::InvalidLocation;
            }

            // C++ BuildAssistant.cpp:333-343 — clear removable, then
            // moveObjectsForConstruction. Human owners abort (no charge, no
            // structure) when leftover/C++ would return FALSE.
            self.clear_removable_for_construction(location, orientation, template_name);
            let place_r = self
                .game_logic
                .structure_place_radius_for_template(template_name);
            if !self
                .game_logic
                .move_objects_for_construction(location, place_r, Some(unit_id))
                && self.game_logic.player_is_human(self.current_player_id)
            {
                return CommandResult::InvalidLocation;
            }

            {
                let Some(player) = self.game_logic.get_player_mut(self.current_player_id) else {
                    continue;
                };

                if !player.spend_resources(&build_cost) {
                    return CommandResult::InvalidCommand;
                }
            }

            let building_id = self.game_logic.create_object_under_construction_for_player(
                template_name,
                self.current_player_id,
                location,
            );
            let Some(building_id) = building_id else {
                // Refund on failed placement.
                if let Some(player) = self.game_logic.get_player_mut(self.current_player_id) {
                    player.resources.supplies = player
                        .resources
                        .supplies
                        .saturating_add(build_cost.supplies);
                }
                return CommandResult::InvalidCommand;
            };
            if orientation.abs() > f32::EPSILON {
                // Wave 233: orientation stamp via GameLogic authority API.
                let _ = self
                    .game_logic
                    .unit_command_set_orientation(building_id, orientation);
            }

            // C++ DozerAIUpdate keeps the newly placed structure as the
            // dozer's exclusive construction target.  Construction progress
            // uses that target identity to count only the assigned dozer;
            // an order target (rather than an attack target) preserves the
            // Constructing state while still recording the target.
            if !self
                .game_logic
                .unit_command_set_order_target(unit_id, Some(building_id))
            {
                if let Some(player) = self.game_logic.get_player_mut(self.current_player_id) {
                    player.resources.supplies = player
                        .resources
                        .supplies
                        .saturating_add(build_cost.supplies);
                }
                return CommandResult::InvalidCommand;
            }
            // C++ construct:1717 newTask(DOZER_TASK_BUILD, obj).
            self.game_logic.dozer_new_task_build(unit_id, building_id);
            // C++ WorkerAIUpdate::newTask: leave supply-truck mode / drop dock.
            self.game_logic.worker_exit_supply_for_dozer_task(unit_id);
            // C++ findGoodBuildOrRepairPosition half majorRadius + findPositionAround.
            let (dozer_pos, pad_pos, pad_radius, stored_dock) = {
                let d = self.game_logic.host_object(unit_id);
                let dpos = d.map(|u| u.get_position()).unwrap_or(location);
                let stored = d.and_then(|u| u.dozer_dock_action);
                let (ppos, prad) = self
                    .game_logic
                    .host_object(building_id)
                    .map(|b| (b.get_position(), b.selection_radius))
                    .unwrap_or((location, 0.0));
                (dpos, ppos, prad, stored)
            };
            let approach = stored_dock.unwrap_or_else(|| {
                crate::game_logic::host_repair::dozer_repair_approach_position(
                    dozer_pos, pad_pos, pad_radius,
                )
            });
            let _ = self.path_to_goal_with_state_ignoring(
                unit_id,
                approach,
                AIState::Constructing,
                Some(building_id),
            );
            self.game_logic.queue_picked_unit_voice(
                &[unit_id],
                crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::BuildResponse,
            );
            // C++ GameLogicDispatch.cpp:1400-1403 PlaceBuilding on the constructor.
            self.game_logic.queue_audio_event(
                AudioEventRequest::new(translate_audio_event("PlaceBuilding")).with_object(unit_id),
            );

            debug!(
                "Unit {} building {} at {:?}",
                unit_id.0, template_name, location
            );
            return CommandResult::Success;
        }
        CommandResult::InvalidCommand
    }

    pub(super) fn execute_dozer_construct(
        &mut self,
        units: &[ObjectId],
        template_name: &str,
        location: Vec3,
        orientation: f32,
    ) -> CommandResult {
        self.execute_build(units, template_name, location, orientation)
    }

    pub(super) fn execute_dozer_line(
        &mut self,
        units: &[ObjectId],
        template_name: &str,
        start: Vec3,
        end: Vec3,
    ) -> CommandResult {
        if units.is_empty() {
            return CommandResult::InvalidCommand;
        }

        // C++ BuildAssistant::buildObjectLineNow — majorRadius*2, max 50,
        // march along the normalized start→end vector, stop at first illegal.
        let positions = self.line_build_tiled_positions(units[0], template_name, start, end);
        if positions.is_empty() {
            return CommandResult::InvalidCommand;
        }
        let builder = units[0];
        let delta = end - start;
        let orient = delta.z.atan2(delta.x);
        let mut placed_segments: Vec<(Vec3, ObjectId)> = Vec::new();
        for pos in positions {
            match self.place_line_build_segment(builder, template_name, pos, orient) {
                Ok(building_id) => placed_segments.push((pos, building_id)),
                Err(_) if !placed_segments.is_empty() => {
                    // C++ buildTiledLocations already stopped; a later create
                    // failure should not keep stretching through the line.
                    break;
                }
                Err(_) => {}
            }
        }
        if placed_segments.is_empty() {
            return CommandResult::InvalidCommand;
        }
        self.game_logic.queue_picked_unit_voice(
            &[builder],
            crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::BuildResponse,
        );
        // C++ GameLogicDispatch.cpp:1400-1403 — one PlaceBuilding after the line.
        self.game_logic.queue_audio_event(
            AudioEventRequest::new(translate_audio_event("PlaceBuilding")).with_object(builder),
        );

        // C++ residual contract: buildObjectLineNow (BuildAssistant.cpp:430-454)
        // places every scaffold first; the dozers then WALK to their segments —
        // AI_MOVE to a `findGoodBuildOrRepairPosition` approach point
        // (DozerAIUpdate.cpp:1855-1894) beside the segment footprint and only
        // flip to AI_CONSTRUCT/Constructing on arrival (DOZER_DO_BUILD_AT_DOCK,
        // DozerAIUpdate.cpp:499-507). Each selected worker owns one segment and
        // the walk legs are installed AFTER placement so placement shoves
        // (moveObjectsForConstruction) cannot clobber the segment destinations.
        let workers: Vec<ObjectId> = units
            .iter()
            .copied()
            .filter(|&id| {
                self.game_logic.host_object(id).is_some_and(|unit| {
                    unit.can_construct()
                        && unit.owner_player_id == Some(self.current_player_id)
                })
            })
            .collect();
        if !workers.is_empty() {
            for (seg_idx, &(seg_pos, building_id)) in placed_segments.iter().enumerate() {
                let worker = workers[seg_idx % workers.len()];
                if !self
                    .game_logic
                    .unit_command_set_order_target(worker, Some(building_id))
                {
                    continue;
                }
                // C++ construct:1717 newTask(DOZER_TASK_BUILD, obj).
                self.game_logic.dozer_new_task_build(worker, building_id);
                // C++ WorkerAIUpdate::newTask: leave supply-truck mode / drop dock.
                self.game_logic.worker_exit_supply_for_dozer_task(worker);
                let (worker_pos, pad_radius) = {
                    match self.game_logic.host_object(building_id) {
                        Some(b) => (b.get_position(), b.selection_radius),
                        None => continue,
                    }
                };
                let approach = self.game_logic.find_good_build_or_repair_position(
                    worker_pos,
                    seg_pos,
                    pad_radius,
                    false,
                    None,
                    Some(worker),
                );
                // C++ line build: the dozer WALKS to its segment (AI_MOVE ⇒
                // Moving) and only flips to AI_CONSTRUCT/Constructing on
                // arrival (DozerAIUpdate DOZER_DO_BUILD_AT_DOCK, cpp:499-507).
                let _ = self.path_to_goal_with_state_ignoring(
                    worker,
                    approach,
                    AIState::Moving,
                    Some(building_id),
                );
            }
        }

        CommandResult::Success
    }

    /// C++ `BuildAssistant::buildTiledLocations` (BuildAssistant.cpp:1090-1191).
    fn line_build_tiled_positions(
        &self,
        builder_id: ObjectId,
        template_name: &str,
        start: Vec3,
        end: Vec3,
    ) -> Vec<Vec3> {
        use crate::game_logic::host_structure_economy_residual::MAX_LINE_BUILD_OBJECTS;
        let spacing = self.line_build_tile_spacing(template_name);
        let dx = end.x - start.x;
        let dz = end.z - start.z;
        let len = (dx * dx + dz * dz).sqrt();
        let mut tiles_needed = if len < 1.0 {
            1usize
        } else {
            ((len / spacing).floor() as usize).saturating_add(1)
        };
        tiles_needed = tiles_needed.clamp(1, MAX_LINE_BUILD_OBJECTS as usize);
        let team = self
            .game_logic
            .host_object(builder_id)
            .map(|o| o.team)
            .unwrap_or(Team::USA);
        let mut positions = Vec::with_capacity(tiles_needed);
        positions.push(start);
        if tiles_needed == 1 || len < 1.0 {
            return positions;
        }
        let inv = 1.0 / len;
        let dir_x = dx * inv;
        let dir_z = dz * inv;
        for i in 1..tiles_needed {
            let pos = Vec3::new(
                start.x + dir_x * spacing * i as f32,
                start.y,
                start.z + dir_z * spacing * i as f32,
            );
            if !self.game_logic.is_location_legal_to_build_for_builder(
                team,
                pos,
                template_name,
                Some(builder_id),
            ) {
                break;
            }
            positions.push(pos);
        }
        positions
    }

    /// C++ `what->getTemplateGeometryInfo().getMajorRadius() * 2.0f`.
    fn line_build_tile_spacing(&self, template_name: &str) -> f32 {
        if let Some(tmpl) = self.game_logic.get_templates().get(template_name) {
            if tmpl.geometry_info.authored && tmpl.geometry_info.major_radius > 0.5 {
                return tmpl.geometry_info.major_radius * 2.0;
            }
        }
        20.0
    }

    /// C++ `buildObjectNow` else-arm: UNDER_CONSTRUCTION then same-frame complete.
    /// Line-build templates never assign a dozer (`isLineBuildTemplate` skip).
    fn place_line_build_segment(
        &mut self,
        builder_id: ObjectId,
        template_name: &str,
        location: Vec3,
        orientation: f32,
    ) -> Result<ObjectId, CommandResult> {
        if !self.validate_build_location(location) {
            return Err(CommandResult::InvalidLocation);
        }
        let (base_cost, is_structure) = match self.game_logic.get_templates().get(template_name) {
            Some(t) => (t.build_cost, t.is_kind_of(KindOf::Structure)),
            None => return Err(CommandResult::InvalidCommand),
        };
        let build_cost = self.calc_cost_to_build(template_name, base_cost);
        if !is_structure {
            return Err(CommandResult::InvalidCommand);
        }
        let team = match self.game_logic.host_object(builder_id) {
            Some(unit)
                if unit.can_construct() && unit.owner_player_id == Some(self.current_player_id) =>
            {
                unit.team
            }
            _ => return Err(CommandResult::InvalidCommand),
        };
        if !self.game_logic.is_location_legal_to_build_for_builder(
            team,
            location,
            template_name,
            Some(builder_id),
        ) {
            return Err(CommandResult::InvalidLocation);
        }
        // C++ BuildAssistant.cpp:333-343 — same human refuse as buildObjectNow.
        self.clear_removable_for_construction(location, orientation, template_name);
        let place_r = self
            .game_logic
            .structure_place_radius_for_template(template_name);
        if !self
            .game_logic
            .move_objects_for_construction(location, place_r, Some(builder_id))
            && self.game_logic.player_is_human(self.current_player_id)
        {
            return Err(CommandResult::InvalidLocation);
        }
        {
            let Some(player) = self.game_logic.get_player_mut(self.current_player_id) else {
                return Err(CommandResult::InvalidCommand);
            };
            if !player.spend_resources(&build_cost) {
                return Err(CommandResult::InvalidCommand);
            }
        }
        let Some(building_id) = self.game_logic.create_object_under_construction_for_player(
            template_name,
            self.current_player_id,
            location,
        ) else {
            if let Some(player) = self.game_logic.get_player_mut(self.current_player_id) {
                player.resources.supplies = player
                    .resources
                    .supplies
                    .saturating_add(build_cost.supplies);
            }
            return Err(CommandResult::InvalidCommand);
        };
        if orientation.abs() > f32::EPSILON {
            let _ = self
                .game_logic
                .unit_command_set_orientation(building_id, orientation);
        }
        // C++ BuildAssistant.cpp:368 newObject keeps ActiveBody initialHealth,
        // then onStructureCreated + onStructureConstructionComplete same frame.
        // Live create_under_construction starts at 1 HP for dozer scaffolds.
        if let Some(obj) = self.game_logic.host_object_mut(building_id) {
            obj.health.current = obj.health.maximum;
        }
        let _ = self.game_logic.force_complete_construction(building_id);
        self.game_logic
            .notify_structure_construction_complete(building_id);
        Ok(building_id)
    }

    pub(super) fn execute_cancel_construction(
        &mut self,
        object_id: ObjectId,
        player_id: u32,
    ) -> CommandResult {
        let (template_name, base_supplies, reconstructing) = {
            let Some(obj) = self.game_logic.host_object(object_id) else {
                return CommandResult::InvalidTarget;
            };
            if obj.owner_player_id != Some(player_id) {
                return CommandResult::InvalidTarget;
            }
            // C++ MSG_DOZER_CANCEL_CONSTRUCT: must be under construction, not sold.
            if !obj.status.under_construction || obj.status.sold {
                return CommandResult::InvalidCommand;
            }
            (
                obj.template_name.clone(),
                obj.thing.template.build_cost.supplies,
                obj.status.reconstructing,
            )
        };
        // C++ GameLogicDispatch.cpp:1430-1435: calcCostToBuild unless reconstructing.
        let refund = if reconstructing {
            0
        } else {
            self.game_logic
                .modified_build_cost_supplies(player_id, &template_name, base_supplies)
        };
        if refund > 0 {
            if let Some(player) = self.game_logic.get_player_mut(player_id) {
                player.resources.supplies = player.resources.supplies.saturating_add(refund);
            }
        }
        // C++ killing the building causes dozer cancelTask residual.
        self.game_logic.cancel_dozers_building(object_id);
        self.game_logic.destroy_object(object_id);
        debug!("Canceled construction of object {}", object_id.0);
        CommandResult::Success
    }

    /// C++ `ThingTemplate::calcCostToBuild(player)` for dozer place/cancel.
    fn calc_cost_to_build(
        &self,
        template_name: &str,
        base: crate::game_logic::Resources,
    ) -> crate::game_logic::Resources {
        let mut cost = base;
        cost.supplies = self.game_logic.modified_build_cost_supplies(
            self.current_player_id,
            template_name,
            base.supplies,
        );
        cost
    }

    pub(super) fn execute_resume_construction(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        // C++ MSG_RESUME_CONSTRUCTION / groupResumeConstruction residual.
        if self.game_logic.resume_construction(units, target_id) {
            // C++ CommandXlat.cpp:449-456 MSG_RESUME_CONSTRUCTION → VoiceBuildResponse.
            self.game_logic.queue_picked_unit_voice(
                units,
                crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::BuildResponse,
            );
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ BuildAssistant::isRemovableForConstruction (BuildAssistant.cpp:1331-1358).
    fn is_removable_for_construction(obj: &crate::game_logic::Object) -> bool {
        use game_engine::common::system::kind_of::KindOfMask;
        if obj.is_kind_of(KindOf::Inert) {
            return false;
        }
        let leftover = leftover_kindof_bits(&obj.template_name);
        if leftover & KindOfMask::INERT.bits() != 0 {
            return false;
        }
        if obj.is_kind_of(KindOf::Shrubbery) || obj.is_kind_of(KindOf::ClearedByBuild) {
            return true;
        }
        if leftover & (KindOfMask::SHRUBBERY.bits() | KindOfMask::CLEARED_BY_BUILD.bits()) != 0 {
            return true;
        }
        obj.status.effectively_dead
    }

    /// C++ BuildAssistant::clearRemovableForConstruction +
    /// TerrainVisual::removeTreesAndPropsForConstruction.
    fn clear_removable_for_construction(
        &mut self,
        location: Vec3,
        angle: f32,
        template_name: &str,
    ) {
        let (major, minor, is_box) = self.game_logic.structure_place_footprint(template_name);
        let place_r = if is_box {
            (major * major + minor * minor).sqrt()
        } else {
            major
        };
        let mut to_destroy: Vec<ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.host_objects() {
            if obj.is_kind_of(KindOf::AlwaysSelectable) {
                continue;
            }
            if !Self::is_removable_for_construction(obj) {
                continue;
            }
            let p = obj.get_position();
            let dx = p.x - location.x;
            let dz = p.z - location.z;
            let r = obj.selection_radius.max(1.0);
            if dx * dx + dz * dz <= (place_r + r) * (place_r + r) {
                to_destroy.push(id);
            }
        }
        for id in to_destroy {
            self.game_logic.destroy_object(id);
        }
        #[cfg(feature = "game_client")]
        {
            if let Ok(mut guard) = game_client::terrain::terrain_visual::get_terrain_visual() {
                if let Some(visual) = guard.as_mut() {
                    visual.remove_trees_and_props_for_construction(
                        [location.x, location.z, location.y],
                        major,
                        minor,
                        is_box,
                        angle,
                    );
                }
            }
        }
    }
}

fn leftover_kindof_bits(template_name: &str) -> u128 {
    let Some(guard) = game_engine::common::thing::thing_factory::try_get_thing_factory() else {
        return 0;
    };
    let Some(factory) = guard.as_ref() else {
        return 0;
    };
    factory
        .find_template(template_name, false)
        .map(|tmpl| tmpl.get_kindof_bits())
        .unwrap_or(0)
}
