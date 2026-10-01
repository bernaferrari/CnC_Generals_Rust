//! Convert to Hijacked Vehicle Crate Collision Module
//!
//! A crate (actually a hijacker - mobile crate) makes the target vehicle switch
//! sides and hides the hijacker inside. This mirrors the C++ Hijacker behavior.

use crate::common::ObjectID;
use serde::{Deserialize, Serialize};

fn resolve_crate_object(id: ObjectID) -> Option<ObjectID> {
    if id == crate::common::INVALID_ID {
        return None;
    }
    crate::object::registry::OBJECT_REGISTRY.contains(id).then_some(id)
}

use crate::common::{
    FieldParse, GameError, KindOf, ObjectStatusMaskType, ObjectStatusTypes, Relationship,
    kindof_from_name,
};
use crate::helpers::{EvaEvent, TheAudio, TheEva, TheGameLogic, TheRadar};
use crate::modules::{AIUpdateInterface, BehaviorModuleInterface};
use crate::object::collide::COLLISION_MANAGER;
use crate::object::collide::Coord3D as CollideCoord3D;
use crate::object::collide::LegacyCollideAdapter;
use crate::object::collide::crate_collide::crate_collide::{
    CrateCollide as LegacyCrateCollide, CrateCollideModuleData as LegacyCrateCollideModuleData,
};
use crate::object::collide::crate_collide::*;
use crate::object::drawable::DrawableArcExt;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::update::ai_update::dozer_ai_update::DozerTask;
use crate::scripting::engine::transfer_object_name;
use game_engine::common::ini::{FieldParse as IniFieldParse, INI, INIError};
use game_engine::common::thing::module::Module;

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
        object: &ObjectID,
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

        Ok(OBJECT_REGISTRY
            .with_object(other, |other| {
                if !self.base.is_valid_to_execute(other) {
                    return false;
                }

                if other.is_effectively_dead() {
                    return false;
                }

                if other.is_kind_of(KindOf::ImmuneToCapture)
                    || other.is_kind_of(KindOf::Aircraft)
                    || other.is_kind_of(KindOf::Boat)
                    || other.is_kind_of(KindOf::Drone)
                {
                    return false;
                }

                if other.test_status(ObjectStatusTypes::Hijacked) {
                    return false;
                }

                // Only hijack enemy objects.
                let Ok(hijacker_id) = self.base.get_object() else {
                    return false;
                };
                let enemies = OBJECT_REGISTRY
                    .with_object(hijacker_id, |hijacker| {
                        hijacker.relationship_to(other) != Relationship::Enemies
                    })
                    .unwrap_or(false);
                if !enemies {
                    return false;
                }

                // Empty transports only.
                if other.is_kind_of(KindOf::Transport)
                    && other
                        .get_contain()
                        .is_some_and(|contain| contain.get_contained_count() > 0)
                {
                    return false;
                }

                true
            })
            .unwrap_or(false))
    }

    fn execute_crate_behavior(&mut self, other_id: ObjectID) -> Result<bool, GameError> {
        if resolve_crate_object(other_id).is_none() {
            return Ok(false);
        }

        let hijacker_id = self.base.get_object().map_err(GameError::from)?;

        // Require AI goal match to avoid accidental hijack.
        let goal_id = OBJECT_REGISTRY
            .with_object(hijacker_id, |hijacker| {
                hijacker
                    .get_ai_update_interface()
                    .map(|ai| ai.get_goal_object_id())
            })
            .flatten();
        if goal_id != Some(other_id) {
            return Ok(false);
        }

        // C++ feedback calls are void side effects; hijack still completes if they fail.
        let _ = OBJECT_REGISTRY.with_object(other_id, |other| {
            let _ = TheRadar::try_infiltration_event(other);
            if other.is_locally_controlled() {
                let _ = TheEva::set_should_play(EvaEvent::VehicleStolen);
            }
        });

        // Transfer ownership to hijacker's team.
        let new_team = OBJECT_REGISTRY
            .with_object(hijacker_id, |hijacker| hijacker.get_controlling_player())
            .flatten()
            .and_then(|player| {
                crate::player::with_player(player, |p| p.get_default_team_id())
            })
            .flatten();
        if let Some(team) = new_team {
            let _ = OBJECT_REGISTRY.with_object_mut(other_id, |other| {
                other.set_team(Some(team))
            });
        }

        // Mark target as hijacked.
        let _ = OBJECT_REGISTRY.with_object_mut(other_id, |other| {
            other.set_status(ObjectStatusMaskType::HIJACKED, true);
        });

        // Stop any AI activity on target.
        let _ = OBJECT_REGISTRY.with_object_mut(other_id, |other| {
            let pos = *other.get_position();
            if let Some(ai) = other.get_ai_update_interface_mut() {
                let _ = ai.ai_move_to_position(&pos);
                let _ = ai.ai_idle();
                if let Some(dozer_ai) = ai.get_dozer_ai_update_interface_mut() {
                    for task in [DozerTask::Build, DozerTask::Repair, DozerTask::Fortify] {
                        dozer_ai.cancel_task(task);
                    }
                }
            }
        });

        // Play hijack driver audio (event name from C++ data).
        if let Some(audio) = TheAudio::get() {
            let mut event = crate::common::audio::AudioEventRts::new("HijackDriver");
            event.set_object_id(hijacker_id);
            audio.add_audio_event(&event);
        }

        // Transfer script name and veterancy to target (highest wins).
        let hijacker_name = OBJECT_REGISTRY
            .with_object(hijacker_id, |hijacker| hijacker.get_name().clone())
            .unwrap_or_default();
        if !hijacker_name.is_empty() {
            let _ = transfer_object_name(&hijacker_name, other_id);
        }

        if let (Some(target_level), Some(hijacker_level)) = (
            OBJECT_REGISTRY.with_object(other_id, |other| other.get_veterancy_level()),
            OBJECT_REGISTRY.with_object(hijacker_id, |hijacker| hijacker.get_veterancy_level()),
        ) {
            let highest_level = target_level.max(hijacker_level);
            let _ = OBJECT_REGISTRY.with_object_mut(hijacker_id, |hijacker| {
                hijacker.set_veterancy_level_with_side_effects(highest_level, false);
            });
            let _ = OBJECT_REGISTRY.with_object_mut(other_id, |other| {
                other.set_veterancy_level_with_side_effects(highest_level, false);
            });
        }

        // Only a definite "cannot eject" destroys the hijacker. A missing object keeps the rider.
        if self.target_supports_eject_pilot(other_id) == Ok(false) {
            let _ = TheGameLogic::destroy_object_by_id(hijacker_id);
            return Ok(true);
        }

        let target_id = other_id;
        let mut configured = false;
        let _ = OBJECT_REGISTRY.with_object_mut(hijacker_id, |hijacker| {
            configured = hijacker
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
                for behavior in hijacker.get_behavior_modules_mut() {
                    let Some(hijacker_update) = behavior.get_hijacker_control_interface() else {
                        continue;
                    };
                    hijacker_update.configure_hijacked_vehicle(target_id);
                    configured = true;
                    break;
                }
            }
        });

        if configured {
            let _ = OBJECT_REGISTRY.with_object_mut(hijacker_id, |hijacker| {
                let _ = hijacker.on_contained_by(target_id);
                hijacker.set_status(ObjectStatusMaskType::NO_COLLISIONS, true);
                hijacker.set_status(ObjectStatusMaskType::MASKED, true);
                hijacker.set_status(ObjectStatusMaskType::UNSELECTABLE, true);
            });
        }

        let _ = OBJECT_REGISTRY.with_object_mut(hijacker_id, |hijacker| {
            hijacker.leave_group();
            if let Some(ai) = hijacker.get_ai_update_interface_mut() {
                let _ = ai.ai_idle();
            }
            let _ = COLLISION_MANAGER.unregister_object(hijacker_id);
            if let Some(drawable) = hijacker.get_drawable() {
                let _ = drawable.set_drawable_hidden(true);
            }
        });

        let _ = OBJECT_REGISTRY.with_object(hijacker_id, |hijacker| {
            let vision = hijacker.get_vision_range();
            let shroud = hijacker.get_shroud_clearing_range();
            OBJECT_REGISTRY.with_object_mut(other_id, |other| {
                other.set_vision_range(vision);
                other.set_shroud_clearing_range(shroud);
            });
        });

        // By returning FALSE, we will not remove the object (Hijacker).
        Ok(false)
    }

    fn target_supports_eject_pilot(&self, other_id: ObjectID) -> Result<bool, GameError> {
        if resolve_crate_object(other_id).is_none() {
            return Ok(false);
        }

        Ok(OBJECT_REGISTRY
            .with_object_mut(other_id, |other| {
                other
                    .get_behavior_modules_mut()
                    .into_iter()
                    .any(|module| module.get_eject_pilot_die_interface().is_some())
            })
            .unwrap_or(false))
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
            let _ = OBJECT_REGISTRY.with_object(other_id, |other| {
                self.base.finish_execution_attempt(other, success)
            });
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
        ConvertToHijackedVehicleCrateCollide::is_valid_to_execute(self, other_id)
    }

    fn execute_crate_behavior(&mut self, other_id: ObjectID) -> Result<bool, GameError> {
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
        let object_id: ObjectID = 77_200;
        let module = ConvertToHijackedVehicleCrateCollide::new(
            &object_id,
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
