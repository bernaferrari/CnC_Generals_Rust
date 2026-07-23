//! Sabotage Power Plant Crate Collide Module
//!
//! A crate (actually a saboteur - mobile crate) that makes the target power plant lose power.
//! Author: Kris Morness, June 2003 (original C++), converted to Rust

use crate::common::ObjectID;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, RwLock};

fn resolve_crate_object(
    id: ObjectID,
) -> Option<std::sync::Arc<std::sync::RwLock<crate::object::Object>>> {
    if id == crate::common::INVALID_ID {
        return None;
    }
    crate::helpers::TheGameLogic::find_object_by_id(id)
        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
}

// Import types that would be defined in other modules
use crate::ai::*;
use crate::common::*;
use crate::object::collide::crate_collide::crate_collide::{
    CrateCollide as LegacyCrateCollide, CrateCollideModuleData as LegacyCrateCollideModuleData,
};
use crate::object::collide::crate_collide::*;
use crate::object::collide::Coord3D as CollideCoord3D;
use crate::object::collide::LegacyCollideAdapter;
use crate::object::*;
use game_engine::common::ini::{FieldParse as IniFieldParse, INIError, INI};

/// Module data for sabotage power plant crate collide behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SabotagePowerPlantCrateCollideModuleData {
    /// Base crate collide module data
    pub base: LegacyCrateCollideModuleData,
    /// Duration of power sabotage effect in frames
    pub power_sabotage_frames: u32,
}

impl Default for SabotagePowerPlantCrateCollideModuleData {
    fn default() -> Self {
        Self {
            base: LegacyCrateCollideModuleData::default(),
            power_sabotage_frames: 0,
        }
    }
}

impl SabotagePowerPlantCrateCollideModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SABOTAGE_POWER_PLANT_CRATE_COLLIDE_FIELDS)
    }

    /// Build field parser for INI configuration
    pub fn build_field_parse() -> Vec<FieldParse> {
        let mut fields = LegacyCrateCollideModuleData::build_field_parse();
        fields.extend(vec![FieldParse::new(
            "SabotagePowerDuration",
            FieldType::DurationUnsignedInt,
            "power_sabotage_frames",
        )]);
        fields
    }
}

fn parse_kind_of_mask(tokens: &[&str]) -> Result<u64, INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }

    let mut mask = 0u64;
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
        mask |= 1u64 << (kind as u32);
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
    data: &mut SabotagePowerPlantCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.required_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbidden_kind_of(
    _ini: &mut INI,
    data: &mut SabotagePowerPlantCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.forbidden_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbid_owner_player(
    _ini: &mut INI,
    data: &mut SabotagePowerPlantCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_forbid_owner_player = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_building_pickup(
    _ini: &mut INI,
    data: &mut SabotagePowerPlantCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_building_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_human_only(
    _ini: &mut INI,
    data: &mut SabotagePowerPlantCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_human_only_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_pickup_science(
    _ini: &mut INI,
    data: &mut SabotagePowerPlantCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    super::parse_crate_pickup_science(&mut data.base, first_token(tokens)?)
}

fn parse_execute_fx(
    _ini: &mut INI,
    data: &mut SabotagePowerPlantCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_fx = Some(first_token(tokens)?.to_string());
    Ok(())
}

fn parse_execute_animation(
    _ini: &mut INI,
    data: &mut SabotagePowerPlantCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execution_animation_template = first_token(tokens)?.to_string();
    Ok(())
}

fn parse_execute_animation_time(
    _ini: &mut INI,
    data: &mut SabotagePowerPlantCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_display_time_seconds = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_z_rise(
    _ini: &mut INI,
    data: &mut SabotagePowerPlantCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_z_rise_per_second = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_fades(
    _ini: &mut INI,
    data: &mut SabotagePowerPlantCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_fades = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_sabotage_power_duration(
    _ini: &mut INI,
    data: &mut SabotagePowerPlantCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.power_sabotage_frames = INI::parse_duration_unsigned_int(first_token(tokens)?)?;
    Ok(())
}

const SABOTAGE_POWER_PLANT_CRATE_COLLIDE_FIELDS: &[IniFieldParse<
    SabotagePowerPlantCrateCollideModuleData,
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
    IniFieldParse {
        token: "SabotagePowerDuration",
        parse: parse_sabotage_power_duration,
    },
];

/// Sabotage Power Plant Crate Collide module
#[derive(Debug)]
pub struct SabotagePowerPlantCrateCollide {
    /// Base crate collide functionality
    pub base: LegacyCrateCollide,
    /// Module-specific data
    pub module_data: Arc<Mutex<SabotagePowerPlantCrateCollideModuleData>>,
}

impl SabotagePowerPlantCrateCollide {
    /// Create new sabotage power plant crate collide module
    pub fn new(
        object: &Arc<RwLock<Object>>,
        module_data: SabotagePowerPlantCrateCollideModuleData,
    ) -> Self {
        Self {
            base: LegacyCrateCollide::from_object_handle(&object, module_data.base.clone()),
            module_data: Arc::new(Mutex::new(module_data)),
        }
    }

    /// Check if this is a valid target for execution
    fn is_valid_to_execute(&self, other_id: ObjectID) -> Result<bool, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        // First check base validation
        if !self.base.is_valid_to_execute(&other) {
            return Ok(false);
        }

        let other_lock = other.read().map_err(|_| GameError::LockError)?;

        // Can't sabotage dead structures
        if other_lock.is_effectively_dead() {
            return Ok(false);
        }

        // We can only sabotage power plants
        if !other_lock.is_kind_of(KindOf::FSPower) {
            return Ok(false);
        }

        // Can only sabotage enemy buildings
        let relationship = self
            .base
            .get_object()
            .map_err(GameError::from)?
            .read()
            .map_err(|_| GameError::LockError)?
            .relationship_to(&other_lock);

        if relationship != Relationship::Enemies {
            return Ok(false);
        }

        Ok(true)
    }

    /// Execute the crate behavior
    fn execute_crate_behavior(&mut self, other_id: ObjectID) -> Result<bool, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        // Check to make sure that the other object is also the goal object in the AIUpdateInterface
        // in order to prevent an unintentional conversion simply by having the terrorist walk too close
        let object = self.base.get_object().map_err(GameError::from)?;
        let object_lock = object.read().map_err(|_| GameError::LockError)?;
        let _target_id = other.read().map_err(|_| GameError::LockError)?.get_id();

        // Check to make sure that the other object is also the goal object in the AIUpdateInterface
        // in order to prevent an unintentional conversion simply by having the terrorist walk too close
        if let Some(ai_update) = object_lock.get_ai_update_interface() {
            let goal_id = ai_update
                .lock()
                .ok()
                .map(|ai_guard| ai_guard.get_goal_object_id());
            if goal_id != Some(_target_id) {
                return Ok(false);
            }
        }

        drop(object_lock);

        // C++ feedback calls are void side effects; sabotage still completes if they fail.
        let _ = TheRadar::try_infiltration_event(other.clone());

        let _ = self
            .base
            .do_sabotage_feedback_fx(&other, SabotageVictimType::PowerPlant);

        // Play eva sound if locally controlled
        {
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if other_lock.is_locally_controlled() {
                let _ = TheEva::set_should_play(EvaEvent::BuildingSabotaged);
            }
        }

        // Set power sabotage duration and trigger power outage
        {
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if let Some(player) = other_lock.get_controlling_player() {
                let module_data = self.module_data.lock().map_err(|_| GameError::LockError)?;
                let sabotage_frame = TheGameLogic::get_frame() + module_data.power_sabotage_frames;
                drop(module_data);

                // Set the duration inside the player's energy class to record the length of the power outage
                player
                    .write()
                    .map_err(|_| GameError::LockError)?
                    .set_power_sabotaged_till_frame(sabotage_frame);

                // Trigger the callback function that will turn everything off
                player
                    .write()
                    .map_err(|_| GameError::LockError)?
                    .on_power_brown_out_change(true)?;

                // Note: Player::update() will check to turn it back on again once the timer expires
            }
        }

        Ok(true)
    }

    /// Check if this is a sabotage building crate collide
    pub fn is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}

impl LegacyCollideAdapter for SabotagePowerPlantCrateCollide {
    fn legacy_on_collide(
        &mut self,
        other_id: crate::common::ObjectID,
        loc: &CollideCoord3D,
        normal: &CollideCoord3D,
    ) -> Result<(), GameError> {
        let _ = (loc, normal);

        if SabotagePowerPlantCrateCollide::is_valid_to_execute(self, other_id)? {
            let success = SabotagePowerPlantCrateCollide::execute_crate_behavior(self, other_id)?;
            if let Some(other) = crate::helpers::TheGameLogic::find_object_by_id(other_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(other_id))
            {
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
        SabotagePowerPlantCrateCollide::is_valid_to_execute(self, other_id)
    }

    fn legacy_is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}

impl CrateCollideModule for SabotagePowerPlantCrateCollide {
    fn is_valid_to_execute(&self, other_id: ObjectID) -> Result<bool, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        SabotagePowerPlantCrateCollide::is_valid_to_execute(self, other_id)
    }

    fn execute_crate_behavior(&mut self, other_id: ObjectID) -> Result<bool, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        SabotagePowerPlantCrateCollide::execute_crate_behavior(self, other_id)
    }

    fn is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}

impl game_engine::common::system::Snapshotable for SabotagePowerPlantCrateCollide {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sabotage_power_duration_parse_from_ini_uses_cpp_duration_frames() {
        let _lock = crate::test_sync::lock();

        let mut data = SabotagePowerPlantCrateCollideModuleData::default();
        let mut ini = INI::new();
        ini.with_inline_source(
            "SabotagePowerDuration = 1500ms\n\
             RequiredKindOf = FS_POWER\n\
             End\n",
            |ini| data.parse_from_ini(ini),
        )
        .expect("sabotage power plant ini parses");

        assert_eq!(data.power_sabotage_frames, 45);
        assert_ne!(
            data.base.required_kind_of & (1u64 << (KindOf::FSPower as u32)),
            0
        );
    }

    #[test]
    fn sabotage_power_duration_rejects_missing_value_like_cpp() {
        let mut data = SabotagePowerPlantCrateCollideModuleData::default();
        let mut ini = INI::new();

        let err = parse_sabotage_power_duration(&mut ini, &mut data, &["="])
            .expect_err("missing duration should fail");

        assert!(matches!(err, INIError::InvalidData));
        assert_eq!(data.power_sabotage_frames, 0);
    }
}
