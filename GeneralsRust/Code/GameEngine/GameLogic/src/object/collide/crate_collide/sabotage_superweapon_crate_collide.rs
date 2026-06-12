//! Sabotage Superweapon Crate Collide Module
//!
//! A crate (actually a saboteur - mobile crate) that resets the timer on the target superweapon.
//! Author: Kris Morness, June 2003 (original C++), converted to Rust

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, RwLock};

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

/// Module data for sabotage superweapon crate collide behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SabotageSuperweaponCrateCollideModuleData {
    /// Base crate collide module data
    pub base: LegacyCrateCollideModuleData,
}

impl Default for SabotageSuperweaponCrateCollideModuleData {
    fn default() -> Self {
        Self {
            base: LegacyCrateCollideModuleData::default(),
        }
    }
}

impl SabotageSuperweaponCrateCollideModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SABOTAGE_SUPERWEAPON_CRATE_COLLIDE_FIELDS)
    }

    /// Build field parser for INI configuration
    pub fn build_field_parse() -> Vec<FieldParse> {
        // This module doesn't have any additional fields
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
    data: &mut SabotageSuperweaponCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.required_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbidden_kind_of(
    _ini: &mut INI,
    data: &mut SabotageSuperweaponCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.forbidden_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbid_owner_player(
    _ini: &mut INI,
    data: &mut SabotageSuperweaponCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_forbid_owner_player = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_building_pickup(
    _ini: &mut INI,
    data: &mut SabotageSuperweaponCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_building_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_human_only(
    _ini: &mut INI,
    data: &mut SabotageSuperweaponCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_human_only_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_pickup_science(
    _ini: &mut INI,
    data: &mut SabotageSuperweaponCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    super::parse_crate_pickup_science(&mut data.base, first_token(tokens)?)
}

fn parse_execute_fx(
    _ini: &mut INI,
    data: &mut SabotageSuperweaponCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_fx = Some(first_token(tokens)?.to_string());
    Ok(())
}

fn parse_execute_animation(
    _ini: &mut INI,
    data: &mut SabotageSuperweaponCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execution_animation_template = first_token(tokens)?.to_string();
    Ok(())
}

fn parse_execute_animation_time(
    _ini: &mut INI,
    data: &mut SabotageSuperweaponCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_display_time_seconds = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_z_rise(
    _ini: &mut INI,
    data: &mut SabotageSuperweaponCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_z_rise_per_second = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_fades(
    _ini: &mut INI,
    data: &mut SabotageSuperweaponCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_fades = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

const SABOTAGE_SUPERWEAPON_CRATE_COLLIDE_FIELDS: &[IniFieldParse<
    SabotageSuperweaponCrateCollideModuleData,
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

/// Sabotage Superweapon Crate Collide module
#[derive(Debug)]
pub struct SabotageSuperweaponCrateCollide {
    /// Base crate collide functionality
    pub base: LegacyCrateCollide,
    /// Module-specific data
    pub module_data: Arc<Mutex<SabotageSuperweaponCrateCollideModuleData>>,
}

impl SabotageSuperweaponCrateCollide {
    /// Create new sabotage superweapon crate collide module
    pub fn new(
        object: Arc<RwLock<Object>>,
        module_data: SabotageSuperweaponCrateCollideModuleData,
    ) -> Self {
        Self {
            base: LegacyCrateCollide::from_object_handle(object, module_data.base.clone()),
            module_data: Arc::new(Mutex::new(module_data)),
        }
    }

    /// Check if this is a valid target for execution
    fn is_valid_to_execute(&self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        // First check base validation
        if !self.base.is_valid_to_execute(&other) {
            return Ok(false);
        }

        let other_lock = other.read().map_err(|_| GameError::LockError)?;

        // Can't sabotage dead structures
        if other_lock.is_effectively_dead() {
            return Ok(false);
        }

        // We can only sabotage superweapon structures
        if !other_lock.is_kind_of(KindOf::FSSuperweapon)
            && !other_lock.is_kind_of(KindOf::FSStrategyCenter)
        {
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
    fn execute_crate_behavior(&mut self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        // Check to make sure that the other object is also the goal object in the AIUpdateInterface
        // in order to prevent an unintentional conversion simply by having the terrorist walk too close
        let object = self.base.get_object().map_err(GameError::from)?;
        let object_lock = object.read().map_err(|_| GameError::LockError)?;
        let other_id = other.read().map_err(|_| GameError::LockError)?.get_id();

        if let Some(ai) = object_lock.get_ai_update_interface() {
            let goal_id = ai
                .lock()
                .ok()
                .and_then(|ai_guard| ai_guard.get_goal_object())
                .and_then(|goal| goal.read().ok().map(|goal_guard| goal_guard.get_id()));
            if goal_id != Some(other_id) {
                return Ok(false);
            }
        }
        drop(object_lock);

        // C++ feedback calls are void side effects; sabotage still completes if they fail.
        let _ = TheRadar::try_infiltration_event(other.clone());

        let _ = self
            .base
            .do_sabotage_feedback_fx(&other, SabotageVictimType::Superweapon);

        // Play eva sound if locally controlled
        {
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if other_lock.is_locally_controlled() {
                let _ = TheEva::set_should_play(EvaEvent::BuildingSabotaged);
            }
        }

        // Reset ALL special powers!
        let other_lock = other.read().map_err(|_| GameError::LockError)?;
        let behavior_modules = other_lock.get_behavior_modules();
        for module in behavior_modules {
            if let Ok(mut module_guard) = module.lock() {
                if let Some(special_power) = module_guard.get_special_power() {
                    special_power.start_power_recharge()?;
                }
            }
        }

        Ok(true)
    }

    /// Check if this is a sabotage building crate collide
    pub fn is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}
impl LegacyCollideAdapter for SabotageSuperweaponCrateCollide {
    fn legacy_on_collide(
        &mut self,
        other: Arc<RwLock<Object>>,
        loc: &CollideCoord3D,
        normal: &CollideCoord3D,
    ) -> Result<(), GameError> {
        let _ = (loc, normal);

        if SabotageSuperweaponCrateCollide::is_valid_to_execute(self, other.clone())? {
            let success =
                SabotageSuperweaponCrateCollide::execute_crate_behavior(self, other.clone())?;
            self.base
                .finish_execution_attempt(&other, success)
                .map_err(GameError::from)?;
        }

        Ok(())
    }

    fn legacy_would_like_to_collide_with(
        &self,
        other: Arc<RwLock<Object>>,
    ) -> Result<bool, GameError> {
        SabotageSuperweaponCrateCollide::is_valid_to_execute(self, other)
    }

    fn legacy_is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}

impl CrateCollideModule for SabotageSuperweaponCrateCollide {
    fn is_valid_to_execute(&self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        SabotageSuperweaponCrateCollide::is_valid_to_execute(self, other)
    }

    fn execute_crate_behavior(&mut self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        SabotageSuperweaponCrateCollide::execute_crate_behavior(self, other)
    }

    fn is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn superweapon_sabotage_parse_from_ini_preserves_cpp_base_fields() {
        let _lock = crate::test_sync::lock();

        let mut data = SabotageSuperweaponCrateCollideModuleData::default();
        let mut ini = INI::new();
        ini.with_inline_source(
            "RequiredKindOf = FS_SUPERWEAPON\n\
             ForbiddenKindOf = FS_STRATEGY_CENTER\n\
             BuildingPickup = true\n\
             ExecuteAnimationTime = 2.75\n\
             End\n",
            |ini| data.parse_from_ini(ini),
        )
        .expect("superweapon sabotage ini parses");

        assert_ne!(
            data.base.required_kind_of & (1u64 << (KindOf::FSSuperweapon as u32)),
            0
        );
        assert_ne!(
            data.base.forbidden_kind_of & (1u64 << (KindOf::FSStrategyCenter as u32)),
            0
        );
        assert!(data.base.is_building_pickup);
        assert!((data.base.execute_animation_display_time_seconds - 2.75).abs() < f32::EPSILON);
    }

    #[test]
    fn superweapon_sabotage_rejects_missing_cpp_base_field_value() {
        let mut data = SabotageSuperweaponCrateCollideModuleData::default();
        let mut ini = INI::new();

        let err = ini
            .with_inline_source("RequiredKindOf =\nEnd\n", |ini| data.parse_from_ini(ini))
            .expect_err("missing kindof value should fail");

        assert!(matches!(err, INIError::InvalidData));
        assert_eq!(data.base.required_kind_of, 0);
    }
}

impl game_engine::common::system::Snapshotable for SabotageSuperweaponCrateCollide {
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
