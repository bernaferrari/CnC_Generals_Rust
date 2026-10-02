//! Convert to Hijacked Vehicle Crate Collision Module
//!
//! A crate (actually a hijacker - mobile crate) makes the target vehicle switch
//! sides and hides the hijacker inside. This mirrors the C++ Hijacker behavior.

use crate::common::ObjectID;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

fn resolve_crate_object(
    id: ObjectID,
) -> Option<std::sync::Arc<std::sync::RwLock<crate::object::Object>>> {
    if id == crate::common::INVALID_ID {
        return None;
    }
    crate::object::registry::OBJECT_REGISTRY.get_object(id)
}

use crate::common::{
    CommandSourceType, FieldParse, KindOf, ObjectStatusMaskType, ObjectStatusTypes,
    kindof_from_name,
};
use crate::helpers::{EvaEvent, TheAudio, TheEva, TheGameLogic, TheRadar};
use crate::modules::AIUpdateInterfaceExt;
use crate::object::Object;
use crate::object::collide::COLLISION_MANAGER;
use crate::object::collide::Coord3D as CollideCoord3D;
use crate::object::collide::LegacyCollideAdapter;
use crate::object::collide::crate_collide::crate_collide::{
    CrateCollide as LegacyCrateCollide, CrateCollideModuleData as LegacyCrateCollideModuleData,
};
use crate::object::collide::crate_collide::*;
use crate::object::drawable::DrawableArcExt;
use crate::object::update::ai_update::dozer_ai_update::DozerTask;
use crate::scripting::engine::transfer_object_name;
use game_engine::common::ini::{FieldParse as IniFieldParse, INI, INIError};

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
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, CONVERT_TO_HIJACKED_VEHICLE_CRATE_COLLIDE_FIELDS)
    }

    /// Build field parser for INI configuration
    pub fn build_field_parse() -> Vec<FieldParse> {
        LegacyCrateCollideModuleData::build_field_parse()
    }
}

fn parse_kind_of_mask(tokens: &[&str]) -> Result<u128, INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }

    let mut mask = 0u128;
    for token in tokens
        .iter()
        .filter(|token| **token != "=")
        .flat_map(|token| token.split('|'))
    {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let Some(kind) = kindof_from_name(token) else {
            return Err(INIError::InvalidData);
        };
        mask |= kind.cpp_mask();
    }
    Ok(mask)
}

fn first_token<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_required_kind_of(
    _ini: &mut INI,
    data: &mut ConvertToHijackedVehicleCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.required_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbidden_kind_of(
    _ini: &mut INI,
    data: &mut ConvertToHijackedVehicleCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.forbidden_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbid_owner_player(
    _ini: &mut INI,
    data: &mut ConvertToHijackedVehicleCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_forbid_owner_player = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_building_pickup(
    _ini: &mut INI,
    data: &mut ConvertToHijackedVehicleCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_building_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_human_only(
    _ini: &mut INI,
    data: &mut ConvertToHijackedVehicleCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_human_only_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_pickup_science(
    _ini: &mut INI,
    data: &mut ConvertToHijackedVehicleCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    super::parse_crate_pickup_science(&mut data.base, first_token(tokens)?)
}

fn parse_execute_fx(
    _ini: &mut INI,
    data: &mut ConvertToHijackedVehicleCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_fx = Some(first_token(tokens)?.to_string());
    Ok(())
}

fn parse_execute_animation(
    _ini: &mut INI,
    data: &mut ConvertToHijackedVehicleCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execution_animation_template = first_token(tokens)?.to_string();
    Ok(())
}

fn parse_execute_animation_time(
    _ini: &mut INI,
    data: &mut ConvertToHijackedVehicleCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_display_time_seconds = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_z_rise(
    _ini: &mut INI,
    data: &mut ConvertToHijackedVehicleCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_z_rise_per_second = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_fades(
    _ini: &mut INI,
    data: &mut ConvertToHijackedVehicleCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_fades = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

const CONVERT_TO_HIJACKED_VEHICLE_CRATE_COLLIDE_FIELDS: &[IniFieldParse<
    ConvertToHijackedVehicleCrateCollideModuleData,
>] = &[
    IniFieldParse {
        token: "RequiredKindOf",
        parse: parse_required_kind_of,
    },
    IniFieldParse {
        token: "ForbiddenKindOf",
        parse: parse_forbidden_kind_of,
    },
    IniFieldParse {
        token: "ForbidOwnerPlayer",
        parse: parse_forbid_owner_player,
    },
    IniFieldParse {
        token: "BuildingPickup",
        parse: parse_building_pickup,
    },
    IniFieldParse {
        token: "HumanOnly",
        parse: parse_human_only,
    },
    IniFieldParse {
        token: "PickupScience",
        parse: parse_pickup_science,
    },
    IniFieldParse {
        token: "ExecuteFX",
        parse: parse_execute_fx,
    },
    IniFieldParse {
        token: "ExecuteAnimation",
        parse: parse_execute_animation,
    },
    IniFieldParse {
        token: "ExecuteAnimationTime",
        parse: parse_execute_animation_time,
    },
    IniFieldParse {
        token: "ExecuteAnimationZRise",
        parse: parse_execute_animation_z_rise,
    },
    IniFieldParse {
        token: "ExecuteAnimationFades",
        parse: parse_execute_animation_fades,
    },
];

/// Hijacker conversion crate collide module.
#[derive(Debug)]
pub struct ConvertToHijackedVehicleCrateCollide {
    /// Base crate collide functionality
    pub base: LegacyCrateCollide,
    /// Module-specific data
    module_data: ConvertToHijackedVehicleCrateCollideModuleData,
}

impl ConvertToHijackedVehicleCrateCollide {
    /// Create new hijacker conversion crate collide module.
    pub fn new(
        object: &Arc<RwLock<Object>>,
        module_data: ConvertToHijackedVehicleCrateCollideModuleData,
    ) -> Self {
        Self {
            base: LegacyCrateCollide::from_object_handle(&object, module_data.base.clone()),
            module_data,
        }
    }

    fn is_valid_to_execute(&self, other_id: ObjectID) -> Result<bool, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

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

    fn execute_crate_behavior(&mut self, other_id: ObjectID) -> Result<bool, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        let hijacker = self.base.get_object().map_err(GameError::from)?;
        let hijacker_lock = hijacker.read().map_err(|_| GameError::LockError)?;
        let hijacker_id = hijacker_lock.get_id();
        let other_id = other.read().map_err(|_| GameError::LockError)?.get_id();

        // Require AI goal match to avoid accidental hijack.
        if let Some(ai) = hijacker_lock.get_ai_update_interface() {
            let goal_id = ai.lock().ok().map(|ai_guard| ai_guard.get_goal_object_id());
            if goal_id != Some(other_id) {
                return Ok(false);
            }
        }

        drop(hijacker_lock);

        // C++ feedback calls are void side effects; hijack still completes if they fail.
        let _ = TheRadar::try_infiltration_event(other.clone());
        {
            if let Ok(other_lock) = other.read() {
                if other_lock.is_locally_controlled() {
                    let _ = TheEva::set_should_play(EvaEvent::VehicleStolen);
                }
            }
        }

        // Transfer ownership to hijacker's team.
        {
            let new_team = hijacker.read().ok().and_then(|hijacker_guard| {
                hijacker_guard
                    .with_controlling_player(|player_guard| player_guard.get_default_team())
                    .unwrap_or(None)
            });

            if let Some(team) = new_team {
                if let Ok(mut other_guard) = other.write() {
                    let _ = other_guard.set_team(Some(team));
                }
            }
        }

        // Mark target as hijacked.
        {
            if let Ok(mut other_guard) = other.write() {
                other_guard.set_status(ObjectStatusMaskType::HIJACKED, true);
            }
        }

        // Stop any AI activity on target.
        {
            if let Ok(other_lock) = other.read() {
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
        }

        // Play hijack driver audio (event name from C++ data).
        if let Some(audio) = TheAudio::get() {
            let mut event = crate::common::audio::AudioEventRts::new("HijackDriver");
            event.set_object_id(hijacker_id);
            audio.add_audio_event(&event);
        }

        // Transfer script name and veterancy to target (highest wins).
        {
            if let Ok(hijacker_guard) = hijacker.read() {
                let hijacker_name = hijacker_guard.get_name().clone();
                let hijacker_level = hijacker_guard.get_veterancy_level();
                let hijacker_has_tracker = hijacker_guard.with_experience_tracker(|_| ()).is_some();
                drop(hijacker_guard);

                if !hijacker_name.is_empty() {
                    transfer_object_name(&hijacker_name, other_id).ok();
                }

                let target_result = other.read().ok().map(|guard| {
                    (
                        guard.with_experience_tracker(|_| ()).is_some(),
                        guard.get_veterancy_level(),
                    )
                });
                if hijacker_has_tracker {
                    if let Some((target_has_tracker, target_level)) = target_result {
                        if target_has_tracker {
                            let highest_level = target_level.max(hijacker_level);
                            if let Ok(mut hijacker_guard) = hijacker.write() {
                                hijacker_guard
                                    .set_veterancy_level_with_side_effects(highest_level, false);
                            }
                            if let Ok(mut target_guard) = other.write() {
                                target_guard
                                    .set_veterancy_level_with_side_effects(highest_level, false);
                            }
                        }
                    }
                }
            }
        }

        // Only a definite "cannot eject" destroys the hijacker. A lock error keeps the rider.
        if self.target_supports_eject_pilot(other_id) == Ok(false) {
            let _ = TheGameLogic::destroy_object_by_id(hijacker_id);
            return Ok(true);
        }

        let target_id = other
            .read()
            .ok()
            .map(|guard| guard.get_id())
            .unwrap_or(crate::common::INVALID_ID);
        let mut configured = false;
        if let Ok(hijacker_guard) = hijacker.read() {
            configured = hijacker_guard
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
                for mut behavior in hijacker_guard.get_behavior_modules() {
                    let Ok(mut behavior) = behavior.access() else {
                        continue;
                    };
                    let Some(hijacker_update) = behavior.get_hijacker_control_interface() else {
                        continue;
                    };
                    hijacker_update.configure_hijacked_vehicle(target_id);
                    configured = true;
                    break;
                }
            }
        }

        if configured {
            if let Ok(mut hijacker_guard) = hijacker.write() {
                let _ = hijacker_guard.on_contained_by(target_id);
                hijacker_guard.set_status(ObjectStatusMaskType::NO_COLLISIONS, true);
                hijacker_guard.set_status(ObjectStatusMaskType::MASKED, true);
                hijacker_guard.set_status(ObjectStatusMaskType::UNSELECTABLE, true);
            }
        }

        if let Ok(mut hijacker_guard) = hijacker.write() {
            hijacker_guard.leave_group();
            if let Some(ai) = hijacker_guard.get_ai_update_interface() {
                ai.ai_idle(CommandSourceType::FromAI);
            }
            let _ = COLLISION_MANAGER.unregister_object(hijacker_id);
            if let Some(drawable) = hijacker_guard.get_drawable() {
                let _ = drawable.set_drawable_hidden(true);
            }
        }

        if let Ok(hijacker_guard) = hijacker.read() {
            let vision = hijacker_guard.get_vision_range();
            let shroud = hijacker_guard.get_shroud_clearing_range();
            drop(hijacker_guard);
            if let Ok(mut other_guard) = other.write() {
                other_guard.set_vision_range(vision);
                other_guard.set_shroud_clearing_range(shroud);
            }
        }

        // By returning FALSE, we will not remove the object (Hijacker).
        Ok(false)
    }

    fn target_supports_eject_pilot(&self, other_id: ObjectID) -> Result<bool, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        let behavior_modules = other
            .read()
            .map_err(|_| GameError::LockError)?
            .get_behavior_modules();
        for mut module in behavior_modules {
            if let Ok(mut guard) = module.access() {
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
        other_id: crate::common::ObjectID,
        loc: &CollideCoord3D,
        normal: &CollideCoord3D,
    ) -> Result<(), GameError> {
        let _ = (loc, normal);

        if ConvertToHijackedVehicleCrateCollide::is_valid_to_execute(self, other_id)? {
            let success =
                ConvertToHijackedVehicleCrateCollide::execute_crate_behavior(self, other_id)?;
            if let Some(other) = crate::object::registry::OBJECT_REGISTRY.get_object(other_id) {
                self.base
                    .finish_execution_attempt(&other, success)
                    .map_err(GameError::from)?;
            }
        }

        Ok(())
    }

    fn legacy_would_like_to_collide_with(
        &self,
        other_id: crate::common::ObjectID,
    ) -> Result<bool, GameError> {
        ConvertToHijackedVehicleCrateCollide::is_valid_to_execute(self, other_id)
    }

    fn legacy_is_hijacked_vehicle_crate_collide(&self) -> bool {
        true
    }
}

impl CrateCollideModule for ConvertToHijackedVehicleCrateCollide {
    fn is_valid_to_execute(&self, other_id: ObjectID) -> Result<bool, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        ConvertToHijackedVehicleCrateCollide::is_valid_to_execute(self, other_id)
    }

    fn execute_crate_behavior(&mut self, other_id: ObjectID) -> Result<bool, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        ConvertToHijackedVehicleCrateCollide::execute_crate_behavior(self, other_id)
    }

    fn is_hijacked_vehicle_crate_collide(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hijacked_vehicle_crate_parse_from_ini_preserves_cpp_base_fields() {
        let _lock = crate::test_sync::lock();

        let mut data = ConvertToHijackedVehicleCrateCollideModuleData::default();
        let mut ini = INI::new();
        ini.with_inline_source(
            "RequiredKindOf = VEHICLE\n\
             ForbiddenKindOf = DRONE|AIRCRAFT\n\
             ExecuteAnimationTime = 1.25\n\
             ExecuteAnimationFades = false\n\
             End\n",
            |ini| data.parse_from_ini(ini),
        )
        .expect("hijacked vehicle crate ini parses");

        assert_ne!(data.base.required_kind_of & (KindOf::Vehicle.cpp_mask()), 0);
        assert_ne!(data.base.forbidden_kind_of & (KindOf::Drone.cpp_mask()), 0);
        assert_ne!(
            data.base.forbidden_kind_of & (KindOf::Aircraft.cpp_mask()),
            0
        );
        assert!((data.base.execute_animation_display_time_seconds - 1.25).abs() < f32::EPSILON);
        assert!(!data.base.execute_animation_fades);
        assert_eq!(data.range_of_effect, 0);
    }

    #[test]
    fn hijacked_vehicle_crate_rejects_missing_cpp_base_field_value() {
        let mut data = ConvertToHijackedVehicleCrateCollideModuleData::default();
        let mut ini = INI::new();

        let err = ini
            .with_inline_source("RequiredKindOf =\nEnd\n", |ini| data.parse_from_ini(ini))
            .expect_err("missing kindof value should fail");

        assert!(matches!(err, INIError::InvalidData));
        assert_eq!(data.base.required_kind_of, 0);
    }

    #[test]
    fn hijacked_vehicle_crate_build_field_parse_omits_non_cpp_range_token() {
        let fields = ConvertToHijackedVehicleCrateCollideModuleData::build_field_parse();
        assert!(fields.iter().any(|field| field.token == "RequiredKindOf"));
        assert!(
            !fields
                .iter()
                .any(|field| field.token == "RangeOfEffect" || field.token == "EffectRange")
        );
    }

    #[test]
    fn hijacked_vehicle_crate_collide_identifies_like_cpp() {
        let object = Arc::new(RwLock::new(Object::new_test(77_200, 100.0)));
        let module = ConvertToHijackedVehicleCrateCollide::new(
            &object,
            ConvertToHijackedVehicleCrateCollideModuleData::default(),
        );

        assert!(crate::object::collide::CollideModule::is_hijacked_vehicle_crate_collide(&module));
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
