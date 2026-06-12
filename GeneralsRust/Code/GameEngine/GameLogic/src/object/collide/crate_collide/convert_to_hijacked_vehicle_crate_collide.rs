//! Convert to Hijacked Vehicle Crate Collision Module
//!
//! A crate (actually a hijacker - mobile crate) makes the target vehicle switch
//! sides and hides the hijacker inside. This mirrors the C++ Hijacker behavior.

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, RwLock};

use crate::common::{
    CommandSourceType, FieldParse, FieldType, KindOf, ObjectStatusMaskType, ObjectStatusTypes,
};
use crate::helpers::{EvaEvent, TheAudio, TheEva, TheGameLogic, TheRadar};
use crate::modules::AIUpdateInterfaceExt;
use crate::object::collide::crate_collide::crate_collide::{
    CrateCollide as LegacyCrateCollide, CrateCollideModuleData as LegacyCrateCollideModuleData,
};
use crate::object::collide::crate_collide::*;
use crate::object::collide::Coord3D as CollideCoord3D;
use crate::object::collide::LegacyCollideAdapter;
use crate::object::collide::COLLISION_MANAGER;
use crate::object::drawable::DrawableArcExt;
use crate::object::update::ai_update::dozer_ai_update::DozerTask;
use crate::object::Object;
use crate::scripting::engine::transfer_object_name;

/// Module data for hijacked vehicle conversion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertToHijackedVehicleCrateCollideModuleData {
    /// Base crate collide module data
    pub base: LegacyCrateCollideModuleData,
    /// Range of effect for the hijacking (currently unused but present in C++)
    pub range_of_effect: u32,
}

impl Default for ConvertToHijackedVehicleCrateCollideModuleData {
    fn default() -> Self {
        Self {
            base: LegacyCrateCollideModuleData::default(),
            range_of_effect: 0,
        }
    }
}

impl ConvertToHijackedVehicleCrateCollideModuleData {
    /// Build field parser for INI configuration
    pub fn build_field_parse() -> Vec<FieldParse> {
        let mut fields = LegacyCrateCollideModuleData::build_field_parse();
        fields.push(FieldParse::new(
            "RangeOfEffect",
            FieldType::UnsignedInt,
            "range_of_effect",
        ));
        fields
    }
}

/// Hijacker conversion crate collide module.
#[derive(Debug)]
pub struct ConvertToHijackedVehicleCrateCollide {
    /// Base crate collide functionality
    pub base: LegacyCrateCollide,
    /// Module-specific data
    pub module_data: Arc<Mutex<ConvertToHijackedVehicleCrateCollideModuleData>>,
}

impl ConvertToHijackedVehicleCrateCollide {
    /// Create new hijacker conversion crate collide module.
    pub fn new(
        object: Arc<RwLock<Object>>,
        module_data: ConvertToHijackedVehicleCrateCollideModuleData,
    ) -> Self {
        Self {
            base: LegacyCrateCollide::from_object_handle(object, module_data.base.clone()),
            module_data: Arc::new(Mutex::new(module_data)),
        }
    }

    fn is_valid_to_execute(&self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        if !self.base.is_valid_to_execute(&other) {
            return Ok(false);
        }

        let other_lock = other.read().map_err(|_| GameError::LockError)?;
        if other_lock.is_effectively_dead() {
            return Ok(false);
        }

        if other_lock.is_kind_of(KindOf::ImmuneToCapture)
            || other_lock.is_kind_of(KindOf::Aircraft)
            || other_lock.is_kind_of(KindOf::Boat)
            || other_lock.is_kind_of(KindOf::Drone)
        {
            return Ok(false);
        }

        if other_lock.test_status(ObjectStatusTypes::Hijacked) {
            return Ok(false);
        }

        // Only hijack enemy objects.
        let hijacker = self.base.get_object().map_err(GameError::from)?;
        let hijacker_lock = hijacker.read().map_err(|_| GameError::LockError)?;
        if hijacker_lock.relationship_to(&other_lock) != Relationship::Enemies {
            return Ok(false);
        }

        // Empty transports only.
        if other_lock.is_kind_of(KindOf::Transport) {
            if let Some(contain) = other_lock.get_contain() {
                if let Ok(contain_guard) = contain.lock() {
                    if contain_guard.get_contained_count() > 0 {
                        return Ok(false);
                    }
                }
            }
        }

        Ok(true)
    }

    fn execute_crate_behavior(&mut self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        let hijacker = self.base.get_object().map_err(GameError::from)?;
        let hijacker_lock = hijacker.read().map_err(|_| GameError::LockError)?;
        let hijacker_id = hijacker_lock.get_id();
        let other_id = other.read().map_err(|_| GameError::LockError)?.get_id();

        // Require AI goal match to avoid accidental hijack.
        if let Some(ai) = hijacker_lock.get_ai_update_interface() {
            let goal_id = ai
                .lock()
                .ok()
                .and_then(|ai_guard| ai_guard.get_goal_object())
                .and_then(|goal| goal.read().ok().map(|goal_guard| goal_guard.get_id()));
            if goal_id != Some(other_id) {
                return Ok(false);
            }
        }

        drop(hijacker_lock);

        // Radar event + EVA feedback.
        TheRadar::try_infiltration_event(other.clone())?;
        {
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if other_lock.is_locally_controlled() {
                TheEva::set_should_play(EvaEvent::VehicleStolen)?;
            }
        }

        // Transfer ownership to hijacker's team.
        {
            let hijacker_guard = hijacker.read().map_err(|_| GameError::LockError)?;
            let new_team = if let Some(player_arc) = hijacker_guard.get_controlling_player() {
                if let Ok(player_guard) = player_arc.read() {
                    player_guard.get_default_team()
                } else {
                    None
                }
            } else {
                None
            };
            drop(hijacker_guard);

            if let Some(team) = new_team {
                let mut other_guard = other.write().map_err(|_| GameError::LockError)?;
                other_guard.set_team(Some(team)).map_err(GameError::from)?;
            }
        }

        // Mark target as hijacked.
        {
            let mut other_guard = other.write().map_err(|_| GameError::LockError)?;
            other_guard.set_status(ObjectStatusMaskType::HIJACKED, true);
        }

        // Stop any AI activity on target.
        {
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if let Some(ai) = other_lock.get_ai_update_interface() {
                let pos = *other_lock.get_position();
                ai.ai_move_to_position(&pos, false, CommandSourceType::FromAI);
                ai.ai_idle(CommandSourceType::FromAI);
                if let Ok(mut ai_guard) = ai.lock() {
                    if let Some(dozer_ai) = ai_guard.get_dozer_ai_update_interface_mut() {
                        for task in [DozerTask::Build, DozerTask::Repair, DozerTask::Fortify] {
                            dozer_ai.cancel_task(task);
                        }
                    }
                }
            }
        }

        // Play hijack driver audio (event name from C++ data).
        if let Some(audio) = TheAudio::get() {
            let mut event = crate::common::audio::AudioEventRts::new("HijackDriver");
            event.set_object_id(hijacker_id);
            audio.add_audio_event(&event);
        }

        // Transfer script name and veterancy to target (highest wins).
        {
            let hijacker_guard = hijacker.read().map_err(|_| GameError::LockError)?;
            let hijacker_name = hijacker_guard.get_name().clone();
            let hijacker_tracker = hijacker_guard.get_experience_tracker();
            drop(hijacker_guard);

            if !hijacker_name.is_empty() {
                transfer_object_name(&hijacker_name, other_id).ok();
            }

            let target_tracker = other
                .read()
                .map_err(|_| GameError::LockError)?
                .get_experience_tracker();
            if let (Some(target_tracker), Some(hijacker_tracker)) =
                (target_tracker, hijacker_tracker)
            {
                let target_level = target_tracker
                    .lock()
                    .map_err(|_| GameError::LockError)?
                    .get_veterancy_level();
                let hijacker_level = hijacker_tracker
                    .lock()
                    .map_err(|_| GameError::LockError)?
                    .get_veterancy_level();
                let highest_level = target_level.max(hijacker_level);

                if let Ok(mut target_tracker_guard) = target_tracker.lock() {
                    target_tracker_guard.set_veterancy_level(highest_level);
                }
                if let Ok(mut hijacker_tracker_guard) = hijacker_tracker.lock() {
                    hijacker_tracker_guard.set_veterancy_level(highest_level);
                }
            }
        }

        // If target cannot eject pilots, destroy hijacker and finish.
        if !self.target_supports_eject_pilot(&other)? {
            // C++ path treats this as fire-and-forget cleanup.
            let _ = TheGameLogic::destroy_object_by_id(hijacker_id);
            return Ok(true);
        }

        // Attach hijacker to vehicle and hide it.
        let hijacker_ai = {
            let mut hijacker_guard = hijacker.write().map_err(|_| GameError::LockError)?;
            hijacker_guard.leave_group();
            hijacker_guard.get_ai_update_interface()
        };
        if let Some(ai) = hijacker_ai {
            ai.ai_idle(CommandSourceType::FromAI);
        }

        {
            let mut hijacker_guard = hijacker.write().map_err(|_| GameError::LockError)?;
            hijacker_guard.on_contained_by(other.clone()).ok();
            hijacker_guard.set_status(ObjectStatusMaskType::NO_COLLISIONS, true);
            hijacker_guard.set_status(ObjectStatusMaskType::MASKED, true);
            hijacker_guard.set_status(ObjectStatusMaskType::UNSELECTABLE, true);
            let _ = COLLISION_MANAGER.unregister_object(hijacker_id);
            if let Some(drawable) = hijacker_guard.get_drawable() {
                let _ = drawable.set_drawable_hidden(true);
            }
        }

        // Configure HijackerUpdate to track the vehicle.
        {
            let target_id = other.read().map_err(|_| GameError::LockError)?.get_id();
            let hijacker_guard = hijacker.read().map_err(|_| GameError::LockError)?;
            let configured = hijacker_guard
                .find_update_module("HijackerUpdate")
                .is_some_and(|module| {
                    module.with_module(|module| {
                        module
                            .get_hijacker_control_interface()
                            .map(|hijacker_update| {
                                hijacker_update.configure_hijacked_vehicle(target_id)
                            })
                            .is_some()
                    })
                });

            if !configured {
                for behavior in hijacker_guard.get_behavior_modules() {
                    let Ok(mut behavior) = behavior.lock() else {
                        continue;
                    };
                    let Some(hijacker_update) = behavior.get_hijacker_control_interface() else {
                        continue;
                    };
                    hijacker_update.configure_hijacked_vehicle(target_id);
                    break;
                }
            }
        }

        // Transfer vision and shroud clearing ranges from hijacker to vehicle.
        {
            let hijacker_guard = hijacker.read().map_err(|_| GameError::LockError)?;
            let vision = hijacker_guard.get_vision_range();
            let shroud = hijacker_guard.get_shroud_clearing_range();
            drop(hijacker_guard);
            let mut other_guard = other.write().map_err(|_| GameError::LockError)?;
            other_guard.set_vision_range(vision);
            other_guard.set_shroud_clearing_range(shroud);
        }

        // Do not destroy hijacker: it is now inside the vehicle.
        Ok(false)
    }

    fn target_supports_eject_pilot(&self, other: &Arc<RwLock<Object>>) -> Result<bool, GameError> {
        let behavior_modules = other
            .read()
            .map_err(|_| GameError::LockError)?
            .get_behavior_modules();
        for module in behavior_modules {
            if let Ok(mut guard) = module.lock() {
                if guard.get_eject_pilot_die_interface().is_some() {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

impl LegacyCollideAdapter for ConvertToHijackedVehicleCrateCollide {
    fn legacy_on_collide(
        &mut self,
        other: Arc<RwLock<Object>>,
        loc: &CollideCoord3D,
        normal: &CollideCoord3D,
    ) -> Result<(), GameError> {
        let _ = (loc, normal);

        if ConvertToHijackedVehicleCrateCollide::is_valid_to_execute(self, other.clone())?
            && ConvertToHijackedVehicleCrateCollide::execute_crate_behavior(self, other.clone())?
        {
            self.base
                .finalize_collection(&other)
                .map_err(GameError::from)?;
        }

        Ok(())
    }

    fn legacy_would_like_to_collide_with(
        &self,
        other: Arc<RwLock<Object>>,
    ) -> Result<bool, GameError> {
        ConvertToHijackedVehicleCrateCollide::is_valid_to_execute(self, other)
    }
}

impl CrateCollideModule for ConvertToHijackedVehicleCrateCollide {
    fn is_valid_to_execute(&self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        ConvertToHijackedVehicleCrateCollide::is_valid_to_execute(self, other)
    }

    fn execute_crate_behavior(&mut self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        ConvertToHijackedVehicleCrateCollide::execute_crate_behavior(self, other)
    }

    fn is_hijacked_vehicle_crate_collide(&self) -> bool {
        true
    }
}

impl game_engine::common::system::Snapshotable for ConvertToHijackedVehicleCrateCollide {
    fn crc(&self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        // C++ parity: versioned xfer entry point (current version 1).
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| err.to_string())?;
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}
