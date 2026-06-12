//! Convert to Car Bomb Crate Collision Module
//!
//! A crate (actually a terrorist - mobile crate) that converts a car into a car bomb,
//! activating its weapon and then activating its AI.
//! Author: Graham Smallwood, March 2002 (original C++), converted to Rust

use std::sync::{Arc, Mutex, RwLock};

use crate::common::{
    kindof_from_name, FieldParse, FieldType, KindOf, ObjectStatusMaskType, ObjectStatusTypes,
};
use crate::effects::FXList;
use crate::helpers::TheFXListStore;
use crate::object::collide::crate_collide::crate_collide::{
    CrateCollide as LegacyCrateCollide, CrateCollideModuleData as LegacyCrateCollideModuleData,
};
use crate::object::collide::crate_collide::*;
use crate::object::collide::Coord3D as CollideCoord3D;
use crate::object::collide::LegacyCollideAdapter;
use crate::object::Object;
use crate::scripting::engine::transfer_object_name;
use crate::weapon::{WeaponSetFlags, WeaponSetType};
use game_engine::common::ini::{FieldParse as IniFieldParse, INIError, INI};

/// Module data for convert to car bomb crate collide behavior
#[derive(Debug, Clone)]
pub struct ConvertToCarBombCrateCollideModuleData {
    /// Base crate collide module data
    pub base: LegacyCrateCollideModuleData,
    /// Range of effect for the conversion (unused in C++ but present)
    pub range_of_effect: u32,
    /// FX list to play when conversion occurs
    pub fx_list: Option<Arc<FXList>>,
}

impl Default for ConvertToCarBombCrateCollideModuleData {
    fn default() -> Self {
        Self {
            base: LegacyCrateCollideModuleData::default(),
            range_of_effect: 0,
            fx_list: None,
        }
    }
}

impl ConvertToCarBombCrateCollideModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, CONVERT_TO_CAR_BOMB_CRATE_COLLIDE_FIELDS)
    }

    /// Build field parser for INI configuration
    pub fn build_field_parse() -> Vec<FieldParse> {
        let mut fields = LegacyCrateCollideModuleData::build_field_parse();
        fields.push(FieldParse::new("FXList", FieldType::String, "fx_list"));
        fields
    }
}

fn first_token<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
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

fn parse_required_kind_of(
    _ini: &mut INI,
    data: &mut ConvertToCarBombCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.required_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbidden_kind_of(
    _ini: &mut INI,
    data: &mut ConvertToCarBombCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.forbidden_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbid_owner_player(
    _ini: &mut INI,
    data: &mut ConvertToCarBombCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_forbid_owner_player = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_building_pickup(
    _ini: &mut INI,
    data: &mut ConvertToCarBombCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_building_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_human_only(
    _ini: &mut INI,
    data: &mut ConvertToCarBombCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_human_only_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_pickup_science(
    _ini: &mut INI,
    data: &mut ConvertToCarBombCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    super::parse_crate_pickup_science(&mut data.base, first_token(tokens)?)
}

fn parse_execute_fx(
    _ini: &mut INI,
    data: &mut ConvertToCarBombCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_fx = Some(first_token(tokens)?.to_string());
    Ok(())
}

fn parse_execute_animation(
    _ini: &mut INI,
    data: &mut ConvertToCarBombCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execution_animation_template = first_token(tokens)?.to_string();
    Ok(())
}

fn parse_execute_animation_time(
    _ini: &mut INI,
    data: &mut ConvertToCarBombCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_display_time_seconds = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_z_rise(
    _ini: &mut INI,
    data: &mut ConvertToCarBombCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_z_rise_per_second = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_fades(
    _ini: &mut INI,
    data: &mut ConvertToCarBombCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_fades = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_conversion_fx_list(
    _ini: &mut INI,
    data: &mut ConvertToCarBombCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.fx_list = TheFXListStore::find_fx_list(first_token(tokens)?);
    Ok(())
}

const CONVERT_TO_CAR_BOMB_CRATE_COLLIDE_FIELDS: &[IniFieldParse<
    ConvertToCarBombCrateCollideModuleData,
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
        token: "FXList",
        parse: parse_conversion_fx_list,
    },
];

/// Convert to Car Bomb crate collide module.
#[derive(Debug)]
pub struct ConvertToCarBombCrateCollide {
    /// Base crate collide functionality
    pub base: LegacyCrateCollide,
    /// Module-specific data
    pub module_data: Arc<Mutex<ConvertToCarBombCrateCollideModuleData>>,
}

impl ConvertToCarBombCrateCollide {
    /// Create new car bomb conversion crate collide module.
    pub fn new(
        object: Arc<RwLock<Object>>,
        module_data: ConvertToCarBombCrateCollideModuleData,
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

        if other_lock.is_kind_of(KindOf::Aircraft) || other_lock.is_kind_of(KindOf::Boat) {
            return Ok(false);
        }

        if other_lock.test_status(ObjectStatusTypes::IsCarBomb) {
            return Ok(false);
        }

        let mut flags = WeaponSetFlags::new();
        flags.set(WeaponSetType::CarBomb);
        let set = other_lock.weapon_set.find_weapon_template_set(&flags);
        let Some(template_set) = set else {
            return Ok(false);
        };
        if !template_set.conditions.test(WeaponSetType::CarBomb) {
            return Ok(false);
        }

        if other_lock.test_weapon_set_flag(WeaponSetType::CarBomb) {
            return Ok(false);
        }

        Ok(true)
    }

    fn execute_crate_behavior(&mut self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        let obj = self.base.get_object().map_err(GameError::from)?;
        let obj_guard = obj.read().map_err(|_| GameError::LockError)?;
        let other_id = other.read().map_err(|_| GameError::LockError)?.get_id();

        // Require AI goal match to avoid accidental conversion.
        if let Some(ai) = obj_guard.get_ai_update_interface() {
            let goal_id = ai
                .lock()
                .ok()
                .and_then(|ai_guard| ai_guard.get_goal_object())
                .and_then(|goal| goal.read().ok().map(|goal_guard| goal_guard.get_id()));
            if goal_id != Some(other_id) {
                return Ok(false);
            }
        }

        // Booby trap check.
        if other
            .read()
            .map_err(|_| GameError::LockError)?
            .check_and_detonate_booby_trap(Some(&obj_guard))
        {
            let other_dead = other
                .read()
                .map_err(|_| GameError::LockError)?
                .is_effectively_dead();
            if other_dead || obj_guard.is_effectively_dead() {
                return Ok(false);
            }
        }

        // Activate car bomb weapon set.
        {
            let mut other_guard = other.write().map_err(|_| GameError::LockError)?;
            other_guard.set_weapon_set_flag(WeaponSetType::CarBomb);
        }

        // Play conversion FX.
        if let Ok(module_guard) = self.module_data.lock() {
            if let Some(fx) = module_guard.fx_list.as_ref() {
                let _ = fx.do_fx_obj(&other, None);
            }
        }

        // Transfer ownership to terrorist's team.
        {
            let new_team = if let Some(player) = obj_guard.get_controlling_player() {
                if let Ok(player_guard) = player.read() {
                    player_guard.get_default_team()
                } else {
                    None
                }
            } else {
                None
            };
            drop(obj_guard);

            if let Some(team) = new_team {
                let mut other_guard = other.write().map_err(|_| GameError::LockError)?;
                other_guard.defect(Some(team), 0);
            }
        }

        // Transfer terrorist name to the car for script control.
        {
            let obj_guard = obj.read().map_err(|_| GameError::LockError)?;
            let owner_name = obj_guard.get_name().clone();
            if !owner_name.is_empty() {
                transfer_object_name(&owner_name, other_id).ok();
            }
        }

        // Transfer vision and shroud clearing ranges.
        {
            let obj_guard = obj.read().map_err(|_| GameError::LockError)?;
            let vision = obj_guard.get_vision_range();
            let shroud = obj_guard.get_shroud_clearing_range();
            drop(obj_guard);

            let mut other_guard = other.write().map_err(|_| GameError::LockError)?;
            other_guard.set_vision_range(vision);
            other_guard.set_shroud_clearing_range(shroud);
        }

        // Mark as car bomb.
        {
            let mut other_guard = other.write().map_err(|_| GameError::LockError)?;
            other_guard.set_status(ObjectStatusMaskType::IS_CAR_BOMB, true);
        }

        // Copy veterancy level.
        {
            let obj_guard = obj.read().map_err(|_| GameError::LockError)?;
            let level = obj_guard.get_veterancy_level();
            drop(obj_guard);

            let other_guard = other.read().map_err(|_| GameError::LockError)?;
            if let Some(exp) = other_guard.get_experience_tracker() {
                if let Ok(mut exp_guard) = exp.lock() {
                    exp_guard.set_veterancy_level(level);
                }
            }
        }

        other
            .read()
            .map_err(|_| GameError::LockError)?
            .refresh_radar_object_from_state();

        Ok(true)
    }
}

impl LegacyCollideAdapter for ConvertToCarBombCrateCollide {
    fn legacy_on_collide(
        &mut self,
        other: Arc<RwLock<Object>>,
        loc: &CollideCoord3D,
        normal: &CollideCoord3D,
    ) -> Result<(), GameError> {
        let _ = (loc, normal);

        if ConvertToCarBombCrateCollide::is_valid_to_execute(self, other.clone())? {
            let success =
                ConvertToCarBombCrateCollide::execute_crate_behavior(self, other.clone())?;
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
        ConvertToCarBombCrateCollide::is_valid_to_execute(self, other)
    }

    fn legacy_is_car_bomb_crate_collide(&self) -> bool {
        true
    }
}

impl CrateCollideModule for ConvertToCarBombCrateCollide {
    fn is_valid_to_execute(&self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        ConvertToCarBombCrateCollide::is_valid_to_execute(self, other)
    }

    fn execute_crate_behavior(&mut self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        ConvertToCarBombCrateCollide::execute_crate_behavior(self, other)
    }
}

impl game_engine::common::system::Snapshotable for ConvertToCarBombCrateCollide {
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
    fn car_bomb_crate_parse_from_ini_preserves_cpp_fx_list_field() {
        let _lock = crate::test_sync::lock();

        TheFXListStore::ensure_fx_list("FX_CarBombConvertParity");

        let mut data = ConvertToCarBombCrateCollideModuleData::default();
        let mut ini = INI::new();
        ini.with_inline_source(
            "FXList = FX_CarBombConvertParity\n\
             ExecuteAnimationTime = 2.5\n\
             End\n",
            |ini| data.parse_from_ini(ini),
        )
        .expect("car bomb crate ini parses");

        let fx = data.fx_list.expect("FXList should resolve");
        assert_eq!(fx.name(), "FX_CarBombConvertParity");
        assert!((data.base.execute_animation_display_time_seconds - 2.5).abs() < f32::EPSILON);
    }

    #[test]
    fn car_bomb_crate_build_field_parse_exposes_cpp_fx_list_token() {
        let fields = ConvertToCarBombCrateCollideModuleData::build_field_parse();
        assert!(fields
            .iter()
            .any(|field| field.token == "FXList" && field.target == "fx_list"));
    }

    #[test]
    fn car_bomb_crate_parse_from_ini_accepts_cpp_kindof_masks() {
        let _lock = crate::test_sync::lock();

        let mut data = ConvertToCarBombCrateCollideModuleData::default();
        let mut ini = INI::new();
        ini.with_inline_source(
            "RequiredKindOf = VEHICLE|INFANTRY\n\
             ForbiddenKindOf = AIRCRAFT\n\
             End\n",
            |ini| data.parse_from_ini(ini),
        )
        .expect("car bomb crate kind-of masks parse");

        assert_ne!(
            data.base.required_kind_of & (1u64 << (KindOf::Vehicle as u32)),
            0
        );
        assert_ne!(
            data.base.required_kind_of & (1u64 << (KindOf::Infantry as u32)),
            0
        );
        assert_ne!(
            data.base.forbidden_kind_of & (1u64 << (KindOf::Aircraft as u32)),
            0
        );
    }

    #[test]
    fn car_bomb_crate_collide_identifies_like_cpp() {
        let object = Arc::new(RwLock::new(Object::new_test(77_100, 100.0)));
        let module = ConvertToCarBombCrateCollide::new(
            object,
            ConvertToCarBombCrateCollideModuleData::default(),
        );

        assert!(crate::object::collide::CollideModule::is_car_bomb_crate_collide(&module));
    }
}
