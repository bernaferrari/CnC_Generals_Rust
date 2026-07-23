//! Sabotage Fake Building Crate Collide Module
//!
//! A crate (actually a saboteur - mobile crate) that destroys a fake building.
//! Author: Kris Morness, July 2003 (original C++), converted to Rust

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

use crate::common::*;
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::object::collide::crate_collide::crate_collide::{
    CrateCollide as LegacyCrateCollide, CrateCollideModuleData as LegacyCrateCollideModuleData,
};
use crate::object::collide::crate_collide::*;
use crate::object::collide::Coord3D as CollideCoord3D;
use crate::object::collide::LegacyCollideAdapter;
use crate::object::*;
use game_engine::common::ini::{FieldParse as IniFieldParse, INIError, INI};

/// Module data for sabotage fake building crate collide behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SabotageFakeBuildingCrateCollideModuleData {
    /// Base crate collide module data
    pub base: LegacyCrateCollideModuleData,
}

impl Default for SabotageFakeBuildingCrateCollideModuleData {
    fn default() -> Self {
        Self {
            base: LegacyCrateCollideModuleData::default(),
        }
    }
}

impl SabotageFakeBuildingCrateCollideModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SABOTAGE_FAKE_BUILDING_CRATE_COLLIDE_FIELDS)
    }

    /// Build field parser for INI configuration
    pub fn build_field_parse() -> Vec<FieldParse> {
        LegacyCrateCollideModuleData::build_field_parse()
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
    data: &mut SabotageFakeBuildingCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.required_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbidden_kind_of(
    _ini: &mut INI,
    data: &mut SabotageFakeBuildingCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.forbidden_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbid_owner_player(
    _ini: &mut INI,
    data: &mut SabotageFakeBuildingCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_forbid_owner_player = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_building_pickup(
    _ini: &mut INI,
    data: &mut SabotageFakeBuildingCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_building_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_human_only(
    _ini: &mut INI,
    data: &mut SabotageFakeBuildingCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_human_only_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_pickup_science(
    _ini: &mut INI,
    data: &mut SabotageFakeBuildingCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    super::parse_crate_pickup_science(&mut data.base, first_token(tokens)?)
}

fn parse_execute_fx(
    _ini: &mut INI,
    data: &mut SabotageFakeBuildingCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_fx = Some(first_token(tokens)?.to_string());
    Ok(())
}

fn parse_execute_animation(
    _ini: &mut INI,
    data: &mut SabotageFakeBuildingCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execution_animation_template = first_token(tokens)?.to_string();
    Ok(())
}

fn parse_execute_animation_time(
    _ini: &mut INI,
    data: &mut SabotageFakeBuildingCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_display_time_seconds = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_z_rise(
    _ini: &mut INI,
    data: &mut SabotageFakeBuildingCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_z_rise_per_second = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_fades(
    _ini: &mut INI,
    data: &mut SabotageFakeBuildingCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_fades = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

const SABOTAGE_FAKE_BUILDING_CRATE_COLLIDE_FIELDS: &[IniFieldParse<
    SabotageFakeBuildingCrateCollideModuleData,
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

/// Sabotage Fake Building Crate Collide module
#[derive(Debug)]
pub struct SabotageFakeBuildingCrateCollide {
    /// Base crate collide functionality
    pub base: LegacyCrateCollide,
    /// Module-specific data
    pub module_data: Arc<Mutex<SabotageFakeBuildingCrateCollideModuleData>>,
}

impl SabotageFakeBuildingCrateCollide {
    /// Create new sabotage fake building crate collide module
    pub fn new(
        object: &Arc<RwLock<Object>>,
        module_data: SabotageFakeBuildingCrateCollideModuleData,
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

        // We can only sabotage fake structures
        if !other_lock.is_kind_of(KindOf::FSFake) {
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
        let other_id = other.read().map_err(|_| GameError::LockError)?.get_id();

        if let Some(ai) = object_lock.get_ai_update_interface() {
            let goal_id = ai.lock().ok().map(|ai_guard| ai_guard.get_goal_object_id());
            if goal_id != Some(other_id) {
                return Ok(false);
            }
        }
        let source_id = object_lock.get_id();
        drop(object_lock);

        // C++ feedback calls do not abort sabotage damage.
        let _ = TheRadar::try_infiltration_event(other.clone());

        let _ = self
            .base
            .do_sabotage_feedback_fx(&other, SabotageVictimType::FakeBuilding);

        // Play eva sound if locally controlled
        {
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if other_lock.is_locally_controlled() {
                let _ = TheEva::set_should_play(EvaEvent::BuildingSabotaged);
            }
        }

        // Apply unresistable damage equal to max health
        let (should_damage, max_health) = {
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if other_lock.get_controlling_player().is_none() {
                (false, 0.0)
            } else if let Some(body) = other_lock.get_body_module() {
                let body_guard = body.lock().map_err(|_| GameError::LockError)?;
                (true, body_guard.get_max_health())
            } else {
                return Err(GameError::ModuleError(
                    "fake building sabotage target has no body module".to_string(),
                ));
            }
        };

        if should_damage {
            let mut damage_info = DamageInfo::with_simple(
                max_health,
                source_id,
                DamageType::Unresistable,
                DeathType::Detonated,
            );
            let mut other_guard = other.write().map_err(|_| GameError::LockError)?;
            let _ = other_guard.attempt_damage(&mut damage_info);
        }

        Ok(true)
    }

    /// Check if this is a sabotage building crate collide
    pub fn is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}

impl LegacyCollideAdapter for SabotageFakeBuildingCrateCollide {
    fn legacy_on_collide(
        &mut self,
        other_id: crate::common::ObjectID,
        loc: &CollideCoord3D,
        normal: &CollideCoord3D,
    ) -> Result<(), GameError> {
        let _ = (loc, normal);

        if SabotageFakeBuildingCrateCollide::is_valid_to_execute(self, other_id)? {
            let success = SabotageFakeBuildingCrateCollide::execute_crate_behavior(self, other_id)?;
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
        SabotageFakeBuildingCrateCollide::is_valid_to_execute(self, other_id)
    }

    fn legacy_is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}

impl CrateCollideModule for SabotageFakeBuildingCrateCollide {
    fn is_valid_to_execute(&self, other_id: ObjectID) -> Result<bool, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        SabotageFakeBuildingCrateCollide::is_valid_to_execute(self, other_id)
    }

    fn execute_crate_behavior(&mut self, other_id: ObjectID) -> Result<bool, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        SabotageFakeBuildingCrateCollide::execute_crate_behavior(self, other_id)
    }

    fn is_sabotage_building_crate_collide(&self) -> bool {
        SabotageFakeBuildingCrateCollide::is_sabotage_building_crate_collide(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_building_sabotage_parse_from_ini_preserves_cpp_base_fields() {
        let _lock = crate::test_sync::lock();

        let mut data = SabotageFakeBuildingCrateCollideModuleData::default();
        let mut ini = INI::new();
        ini.with_inline_source(
            "RequiredKindOf = FS_FAKE\n\
             BuildingPickup = yes\n\
             ExecuteAnimationZRise = 3.25\n\
             End\n",
            |ini| data.parse_from_ini(ini),
        )
        .expect("fake building sabotage ini parses");

        assert_ne!(
            data.base.required_kind_of & (1u64 << (KindOf::FSFake as u32)),
            0
        );
        assert!(data.base.is_building_pickup);
        assert!((data.base.execute_animation_z_rise_per_second - 3.25).abs() < f32::EPSILON);
    }

    #[test]
    fn fake_building_sabotage_rejects_missing_cpp_base_field_value() {
        let mut data = SabotageFakeBuildingCrateCollideModuleData::default();
        let mut ini = INI::new();

        let err = ini
            .with_inline_source("BuildingPickup =\nEnd\n", |ini| data.parse_from_ini(ini))
            .expect_err("missing bool value should fail");

        assert!(matches!(err, INIError::InvalidData));
        assert!(!data.base.is_building_pickup);
    }
}

impl game_engine::common::system::Snapshotable for SabotageFakeBuildingCrateCollide {
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
