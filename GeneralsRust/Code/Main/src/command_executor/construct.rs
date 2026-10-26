//! Dozer/build/resume/cancel construction.
#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
#[path = "construct/admission.rs"]
mod admission;
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum BuildPlacement<'p> {
    Single,
    PreparedLine(&'p HashSet<ObjectId>),
}

impl<'a> CommandExecutor<'a> {
    // === Construction Commands ===

    pub(super) fn execute_build(
        &mut self,
        units: &[ObjectId],
        template_name: &str,
        location: Vec3,
        orientation: f32,
    ) -> gamelogic::GameLogicResult<CommandResult> {
        Ok(self.execute_build_with_placement(
            units,
            template_name,
            location,
            orientation,
            BuildPlacement::Single,
        )?)
    }

    pub(super) fn execute_prepared_dozer_build(
        &mut self,
        builder: ObjectId,
        template_name: &str,
        location: Vec3,
        orientation: f32,
        collision_candidates: &HashSet<ObjectId>,
    ) -> gamelogic::GameLogicResult<CommandResult> {
        Ok(self.execute_build_with_placement(
            &[builder],
            template_name,
            location,
            orientation,
            BuildPlacement::PreparedLine(collision_candidates),
        )?)
    }

    fn execute_build_with_placement(
        &mut self,
        units: &[ObjectId],
        template_name: &str,
        location: Vec3,
        orientation: f32,
        placement: BuildPlacement<'_>,
    ) -> gamelogic::GameLogicResult<CommandResult> {
        let (base_cost, is_structure) = match self.game_logic.get_templates().get(template_name) {
            Some(t) => (t.build_cost, t.is_kind_of(KindOf::Structure)),
            None => {
                return Ok(CommandResult::InvalidCommand);
            }
        };
        let build_cost = self.calc_cost_to_build(template_name, base_cost);

        if !is_structure {
            return Ok(CommandResult::InvalidCommand);
        }

        for &unit_id in units {
            let Some(unit) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !unit.can_construct() {
                continue;
            }
            // C++ GameLogicDispatch.cpp:1363-1383 builds with the selected
            // constructor's OWN controlling player. A wholly ownerless
            // legacy/map world falls back to the team's unique player (mirror
            // can_issue_dock, validate.rs) so builder identity survives old
            // saves; a unit owned by another player never builds here.
            let owner_is_ours = unit.owner_player_id == Some(self.current_player_id);
            if !owner_is_ours {
                continue;
            }
            // C++ BuildAssistant::buildObjectNow:330 rejects a constructor
            // whose live CommandSet lacks the requested build button or whose
            // controlling Player::canBuild refuses it, before clearing or moving.
            if !self
                .game_logic
                .constructor_can_build_template(unit_id, template_name)
            {
                continue;
            }
            if placement == BuildPlacement::Single && !self.validate_build_location(location) {
                return Ok(CommandResult::InvalidLocation);
            }
            let team = unit.team;

            // C++ BuildAssistant CLEAR_PATH residual before charging resources.
            if placement == BuildPlacement::Single
                && !self
                    .game_logic
                    .is_location_legal_to_build_for_builder_with_effects(
                        team,
                        location,
                        template_name,
                        Some(unit_id),
                        &mut self.ui,
                    )
            {
                return Ok(CommandResult::InvalidLocation);
            }

            // C++ BuildAssistant.cpp:333-343 — clear removable, then
            // moveObjectsForConstruction. Human owners abort (no charge, no
            // structure) when leftover/C++ would return FALSE.
            let collision_candidates = match placement {
                BuildPlacement::Single => None,
                BuildPlacement::PreparedLine(ids) => Some(ids),
            };
            self.clear_removable_for_construction_with_candidates(
                location,
                orientation,
                template_name,
                collision_candidates,
            );
            if !self
                .game_logic
                .move_objects_for_construction_with_candidates_with_effects(
                    template_name,
                    location,
                    orientation,
                    Some(unit_id),
                    collision_candidates,
                    &mut self.ui,
                )
                && self.game_logic.player_is_human(self.current_player_id)
            {
                return Ok(CommandResult::InvalidLocation);
            }

            let Some(constructor_birth) = self
                .game_logic
                .dozer_prepare_construct_owned_with_effects(unit_id, &mut self.ui)?
            else {
                return Ok(CommandResult::InvalidCommand);
            };

            {
                let Some(player) = self.game_logic.get_player_mut(self.current_player_id) else {
                    continue;
                };

                if !player.can_afford(&build_cost) {
                    // C++ PlaceEventTranslator.cpp:170-178 — confirmed placement
                    // without enough cash: EVA pulse + HUD text, no structure.
                    if self
                        .game_logic
                        .get_player(self.current_player_id)
                        .is_some_and(|p| p.is_local)
                    {
                        self.game_logic
                            .try_eva_insufficient_funds(self.current_player_id);
                        if let Some(ui) = self.ui.ui.as_deref_mut() {
                            ui.hud_message(
                                "GUI:NotEnoughMoneyToBuild",
                                self.game_logic.get_frame(),
                            );
                        }
                    }
                    return Ok(CommandResult::InvalidCommand);
                }
            }

            let building_id = self.game_logic.create_dozer_scaffold_with_effects(
                template_name,
                self.current_player_id,
                unit_id,
                location,
                orientation,
                build_cost,
                &mut self.ui,
            )?;
            let Some(building_id) = building_id else {
                return Ok(CommandResult::InvalidCommand);
            };
            self.game_logic.dozer_admit_constructed_scaffold_owned(
                unit_id,
                &constructor_birth,
                building_id,
            )?;
            if placement == BuildPlacement::Single {
                self.game_logic.queue_picked_unit_voice(
                    &[unit_id],
                    crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::BuildResponse,
                );
                // C++ GameLogicDispatch.cpp:1400-1403 PlaceBuilding on the constructor.
                self.game_logic.queue_audio_event(
                    AudioEventRequest::new(translate_audio_event("PlaceBuilding"))
                        .with_object(unit_id),
                );
            }

            debug!(
                "Unit {} building {} at {:?}",
                unit_id.0, template_name, location
            );
            return Ok(CommandResult::Success);
        }

        Ok(CommandResult::InvalidCommand)
    }

    pub(super) fn execute_dozer_construct(
        &mut self,
        units: &[ObjectId],
        template_name: &str,
        location: Vec3,
        orientation: f32,
    ) -> gamelogic::GameLogicResult<CommandResult> {
        Ok(self.execute_build(units, template_name, location, orientation)?)
    }

    pub(super) fn execute_cancel_construction(
        &mut self,
        object_id: ObjectId,
        player_id: u32,
    ) -> gamelogic::GameLogicResult<CommandResult> {
        let (template_name, base_supplies, reconstructing) = {
            let Some(obj) = self.game_logic.host_object(object_id) else {
                return Ok(CommandResult::InvalidTarget);
            };
            if obj.owner_player_id != Some(player_id) {
                return Ok(CommandResult::InvalidTarget);
            }
            // C++ MSG_DOZER_CANCEL_CONSTRUCT: must be under construction, not sold.
            if !obj.status.under_construction || obj.status.sold {
                return Ok(CommandResult::InvalidCommand);
            }
            (
                obj.template_name.clone(),
                obj.thing().template.build_cost.supplies,
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
            // GameLogicDispatch.cpp1430-1435: Money::deposit precedes cancel
            // and destruction, including the controlling player's sound cue.
            self.game_logic.player_refund_supplies(player_id, refund);
        }
        // C++ killing the building causes dozer cancelTask residual.
        self.game_logic.cancel_dozers_building(object_id)?;
        self.game_logic
            .destroy_object_with_effects(object_id, &mut self.ui);
        debug!("Canceled construction of object {}", object_id.0);
        Ok(CommandResult::Success)
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
    ) -> gamelogic::GameLogicResult<CommandResult> {
        // C++ MSG_RESUME_CONSTRUCTION / groupResumeConstruction residual.
        if self.game_logic.resume_construction(units, target_id)? {
            // C++ CommandXlat.cpp:449-456 MSG_RESUME_CONSTRUCTION → VoiceBuildResponse.
            self.game_logic.queue_picked_unit_voice(
                units,
                crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::BuildResponse,
            );
            Ok(CommandResult::Success)
        } else {
            Ok(CommandResult::InvalidCommand)
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
    pub(super) fn clear_removable_for_construction(
        &mut self,
        location: Vec3,
        angle: f32,
        template_name: &str,
    ) {
        self.clear_removable_for_construction_with_candidates(location, angle, template_name, None);
    }

    pub(super) fn clear_removable_for_construction_with_candidates(
        &mut self,
        location: Vec3,
        angle: f32,
        template_name: &str,
        eligible_candidates: Option<&HashSet<ObjectId>>,
    ) {
        let (major, minor, is_box) = self.game_logic.structure_place_footprint(template_name);
        let place_r = if is_box {
            (major * major + minor * minor).sqrt()
        } else {
            major
        };
        let mut to_destroy: Vec<ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.host_objects().registered() {
            if eligible_candidates.is_some_and(|ids| !ids.contains(&id)) {
                continue;
            }
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
            self.game_logic
                .destroy_object_with_effects(id, &mut self.ui);
        }
        if let Some(visual) = self.ui.terrain.as_mut() {
            visual.remove_for_construction(
                [location.x, location.z, location.y],
                major,
                minor,
                is_box,
                angle,
            );
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
