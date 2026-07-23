//! Sabotage Internet Center Crate Collide Module
//!
//! A crate (actually a saboteur - mobile crate) that temporarily disables an internet center.
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

// Import types that would be defined in other modules
use crate::ai::*;
use crate::common::*;
use crate::object::collide::crate_collide::crate_collide::{
    CrateCollide as LegacyCrateCollide, CrateCollideModuleData as LegacyCrateCollideModuleData,
};
use crate::object::collide::crate_collide::*;
use crate::object::collide::Coord3D as CollideCoord3D;
use crate::object::collide::LegacyCollideAdapter;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::*;
use game_engine::common::ini::{FieldParse as IniFieldParse, INIError, INI};

/// Module data for sabotage internet center crate collide behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SabotageInternetCenterCrateCollideModuleData {
    /// Base crate collide module data
    pub base: LegacyCrateCollideModuleData,
    /// Duration of sabotage effect in frames
    pub sabotage_frames: u32,
}

impl Default for SabotageInternetCenterCrateCollideModuleData {
    fn default() -> Self {
        Self {
            base: LegacyCrateCollideModuleData::default(),
            sabotage_frames: 0,
        }
    }
}

impl SabotageInternetCenterCrateCollideModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SABOTAGE_INTERNET_CENTER_CRATE_COLLIDE_FIELDS)
    }

    /// Build field parser for INI configuration
    pub fn build_field_parse() -> Vec<FieldParse> {
        let mut fields = LegacyCrateCollideModuleData::build_field_parse();
        fields.extend(vec![FieldParse::new(
            "SabotageDuration",
            FieldType::DurationUnsignedInt,
            "sabotage_frames",
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
    data: &mut SabotageInternetCenterCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.required_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbidden_kind_of(
    _ini: &mut INI,
    data: &mut SabotageInternetCenterCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.forbidden_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbid_owner_player(
    _ini: &mut INI,
    data: &mut SabotageInternetCenterCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_forbid_owner_player = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_building_pickup(
    _ini: &mut INI,
    data: &mut SabotageInternetCenterCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_building_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_human_only(
    _ini: &mut INI,
    data: &mut SabotageInternetCenterCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_human_only_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_pickup_science(
    _ini: &mut INI,
    data: &mut SabotageInternetCenterCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    super::parse_crate_pickup_science(&mut data.base, first_token(tokens)?)
}

fn parse_execute_fx(
    _ini: &mut INI,
    data: &mut SabotageInternetCenterCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_fx = Some(first_token(tokens)?.to_string());
    Ok(())
}

fn parse_execute_animation(
    _ini: &mut INI,
    data: &mut SabotageInternetCenterCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execution_animation_template = first_token(tokens)?.to_string();
    Ok(())
}

fn parse_execute_animation_time(
    _ini: &mut INI,
    data: &mut SabotageInternetCenterCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_display_time_seconds = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_z_rise(
    _ini: &mut INI,
    data: &mut SabotageInternetCenterCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_z_rise_per_second = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_fades(
    _ini: &mut INI,
    data: &mut SabotageInternetCenterCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_fades = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_sabotage_duration(
    _ini: &mut INI,
    data: &mut SabotageInternetCenterCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.sabotage_frames = INI::parse_duration_unsigned_int(first_token(tokens)?)?;
    Ok(())
}

const SABOTAGE_INTERNET_CENTER_CRATE_COLLIDE_FIELDS: &[IniFieldParse<
    SabotageInternetCenterCrateCollideModuleData,
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
        token: "SabotageDuration",
        parse: parse_sabotage_duration,
    },
];

/// Sabotage Internet Center Crate Collide module
#[derive(Debug)]
pub struct SabotageInternetCenterCrateCollide {
    /// Base crate collide functionality
    pub base: LegacyCrateCollide,
    /// Module-specific data
    pub module_data: Arc<Mutex<SabotageInternetCenterCrateCollideModuleData>>,
}

impl SabotageInternetCenterCrateCollide {
    /// Create new sabotage internet center crate collide module
    pub fn new(
        object: &Arc<RwLock<Object>>,
        module_data: SabotageInternetCenterCrateCollideModuleData,
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

        // We can only sabotage internet centers
        if !other_lock.is_kind_of(KindOf::FSInternetCenter) {
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

        // Check AI goal object - only execute if this is the intentional target
        if let Some(ai_update) = object_lock.get_ai_update_interface() {
            let goal_id = ai_update
                .lock()
                .ok()
                .and_then(|ai_guard| ai_guard.get_goal_object())
                .and_then(|goal_obj| goal_obj.read().ok().map(|goal_guard| goal_guard.get_id()));
            if goal_id != Some(other_id) {
                log::debug!(
                    "SabotageInternetCenter: Skipping - target {} is not current goal {:?}",
                    other_id,
                    goal_id
                );
                return Ok(false);
            }
        }
        drop(object_lock);

        // C++ feedback calls are void side effects; sabotage still completes if they fail.
        let _ = TheRadar::try_infiltration_event(other.clone());

        let _ = self
            .base
            .do_sabotage_feedback_fx(&other, SabotageVictimType::InternetCenter);

        // Play eva sound if locally controlled
        {
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if other_lock.is_locally_controlled() {
                let _ = TheEva::set_should_play(EvaEvent::BuildingSabotaged);
            }
        }

        // Calculate disable frame
        let module_data = self.module_data.lock().map_err(|_| GameError::LockError)?;
        let disable_frame = TheGameLogic::get_frame() + module_data.sabotage_frames;
        drop(module_data);

        // Disable all internet center spy visions (they stack) without visually disabling the other centers
        {
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if let Some(controlling_player) = other_lock.get_controlling_player() {
                let player_guard = controlling_player
                    .read()
                    .map_err(|_| GameError::LockError)?;
                player_guard.iterate_object_ids(|obj_id| {
                    disable_internet_center_spy_vision(obj_id, disable_frame)
                })?;
            }
        }

        // Disable the internet center
        {
            let mut other_lock = other.write().map_err(|_| GameError::LockError)?;
            other_lock.set_disabled_until(DisabledType::DisabledHacked, disable_frame);
        }

        // Disable all the hackers inside
        {
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if let Some(contain) = other_lock.get_contain() {
                let contain_guard = contain.lock().map_err(|_| GameError::LockError)?;
                let contained_ids: Vec<ObjectID> = contain_guard.get_contained_objects().to_vec();
                drop(contain_guard);
                for object_id in contained_ids {
                    disable_hacker_id(object_id, disable_frame)?;
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

impl LegacyCollideAdapter for SabotageInternetCenterCrateCollide {
    fn legacy_on_collide(
        &mut self,
        other_id: crate::common::ObjectID,
        loc: &CollideCoord3D,
        normal: &CollideCoord3D,
    ) -> Result<(), GameError> {
        let _ = (loc, normal);

        if SabotageInternetCenterCrateCollide::is_valid_to_execute(self, other_id)? {
            let success =
                SabotageInternetCenterCrateCollide::execute_crate_behavior(self, other_id)?;
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
        SabotageInternetCenterCrateCollide::is_valid_to_execute(self, other_id)
    }

    fn legacy_is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}

impl CrateCollideModule for SabotageInternetCenterCrateCollide {
    fn is_valid_to_execute(&self, other_id: ObjectID) -> Result<bool, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        SabotageInternetCenterCrateCollide::is_valid_to_execute(self, other_id)
    }

    fn execute_crate_behavior(&mut self, other_id: ObjectID) -> Result<bool, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        SabotageInternetCenterCrateCollide::execute_crate_behavior(self, other_id)
    }

    fn is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}

/// Disable hacker callback function
fn disable_hacker(obj: Arc<RwLock<Object>>, frame: u32) -> Result<(), GameError> {
    let mut obj_lock = obj.write().map_err(|_| GameError::LockError)?;
    obj_lock.set_disabled_until(DisabledType::DisabledHacked, frame);
    Ok(())
}

fn disable_hacker_id(object_id: ObjectID, frame: u32) -> Result<(), GameError> {
    let applied = OBJECT_REGISTRY.with_object_mut(object_id, |obj_lock| {
        obj_lock.set_disabled_until(DisabledType::DisabledHacked, frame);
    });
    if applied.is_none() {
        return Err(GameError::LockError);
    }
    Ok(())
}

/// Disable internet center spy vision callback function  
fn disable_internet_center_spy_vision(
    obj_id: crate::common::ObjectID,
    frame: u32,
) -> Result<(), GameError> {
    let applied = crate::object::registry::OBJECT_REGISTRY.with_object(obj_id, |obj_lock| {
        if obj_lock.is_kind_of(KindOf::FSInternetCenter) {
            for module in obj_lock.get_behavior_modules() {
                if let Ok(mut module_guard) = module.lock() {
                    if let Some(spy_vision) = module_guard.get_spy_vision_control_interface() {
                        spy_vision.set_disabled_until_frame(frame);
                    }
                }
            }
        }
    });
    if applied.is_none() {
        return Err(GameError::LockError);
    }
    Ok(())
}

impl game_engine::common::system::Snapshotable for SabotageInternetCenterCrateCollide {
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
    fn sabotage_duration_parse_from_ini_uses_cpp_duration_frames() {
        let _lock = crate::test_sync::lock();

        let mut data = SabotageInternetCenterCrateCollideModuleData::default();
        let mut ini = INI::new();
        ini.with_inline_source(
            "SabotageDuration = 1.5s\n\
             RequiredKindOf = FS_INTERNET_CENTER\n\
             End\n",
            |ini| data.parse_from_ini(ini),
        )
        .expect("sabotage internet center ini parses");

        assert_eq!(data.sabotage_frames, 45);
        assert_ne!(
            data.base.required_kind_of & (1u64 << (KindOf::FSInternetCenter as u32)),
            0
        );
    }

    #[test]
    fn sabotage_duration_rejects_missing_value_like_cpp() {
        let mut data = SabotageInternetCenterCrateCollideModuleData::default();
        let mut ini = INI::new();

        let err = parse_sabotage_duration(&mut ini, &mut data, &["="])
            .expect_err("missing duration should fail");

        assert!(matches!(err, INIError::InvalidData));
        assert_eq!(data.sabotage_frames, 0);
    }

    #[test]
    fn sabotage_internet_center_field_parse_exposes_cpp_token() {
        let fields = SabotageInternetCenterCrateCollideModuleData::build_field_parse();

        assert!(fields.iter().any(|field| {
            field.token == "SabotageDuration" && field.target == "sabotage_frames"
        }));
    }
}
