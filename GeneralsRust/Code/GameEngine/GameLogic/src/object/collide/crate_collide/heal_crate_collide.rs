//! Heal Crate Collision Module
//!
//! A crate that heals every object owned by the player who collects it.

use super::super::{CollideModule, CollisionError, Coord3D, GameObject};
use super::crate_collide::{CrateCollide, CrateCollideBehavior, CrateCollideModuleData};
use crate::common::*;
use crate::helpers::TheAudio;
use crate::object::collide::crate_collide::*;
use game_engine::common::ini::{FieldParse as IniFieldParse, INIError, INI};

/// Configuration data for HealCrateCollide.
///
/// C++ exposes only inherited CrateCollide module data for this module.
#[derive(Debug, Clone)]
pub struct HealCrateCollideModuleData {
    /// Base crate collision data
    pub base: CrateCollideModuleData,
}

impl HealCrateCollideModuleData {
    pub fn new() -> Self {
        Self {
            base: CrateCollideModuleData::new(),
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, HEAL_CRATE_COLLIDE_FIELDS)
    }

    pub fn build_field_parse() -> Vec<FieldParse> {
        CrateCollideModuleData::build_field_parse()
    }
}

impl Default for HealCrateCollideModuleData {
    fn default() -> Self {
        Self::new()
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
    data: &mut HealCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.required_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbidden_kind_of(
    _ini: &mut INI,
    data: &mut HealCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.forbidden_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbid_owner_player(
    _ini: &mut INI,
    data: &mut HealCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_forbid_owner_player = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_building_pickup(
    _ini: &mut INI,
    data: &mut HealCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_building_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_human_only(
    _ini: &mut INI,
    data: &mut HealCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_human_only_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_pickup_science(
    _ini: &mut INI,
    data: &mut HealCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    super::parse_crate_pickup_science(&mut data.base, first_token(tokens)?)
}

fn parse_execute_fx(
    _ini: &mut INI,
    data: &mut HealCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_fx = Some(first_token(tokens)?.to_string());
    Ok(())
}

fn parse_execute_animation(
    _ini: &mut INI,
    data: &mut HealCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execution_animation_template = first_token(tokens)?.to_string();
    Ok(())
}

fn parse_execute_animation_time(
    _ini: &mut INI,
    data: &mut HealCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_display_time_seconds = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_z_rise(
    _ini: &mut INI,
    data: &mut HealCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_z_rise_per_second = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_fades(
    _ini: &mut INI,
    data: &mut HealCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_fades = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

const HEAL_CRATE_COLLIDE_FIELDS: &[IniFieldParse<HealCrateCollideModuleData>] = &[
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

/// Heal Crate Collide implementation.
pub struct HealCrateCollide {
    /// Base crate collision functionality
    base_crate: CrateCollide,
    /// Module-specific configuration
    module_data: HealCrateCollideModuleData,
}

impl HealCrateCollide {
    pub fn new(object_id: ObjectId, module_data: HealCrateCollideModuleData) -> Self {
        Self {
            base_crate: CrateCollide::new(object_id, module_data.base.clone()),
            module_data,
        }
    }

    pub fn get_module_data(&self) -> &HealCrateCollideModuleData {
        &self.module_data
    }

    /// Execute the C++ heal-crate behavior.
    pub fn execute_healing(&mut self, other: &dyn GameObject) -> Result<bool, CollisionError> {
        let Some(other_handle) = other.as_object_handle() else {
            return Ok(false);
        };
        let Some(player) = other_handle
            .read()
            .map_err(|_| CollisionError::InvalidObject("Failed to lock collector".to_string()))?
            .get_controlling_player()
        else {
            return Ok(false);
        };

        if let Ok(mut player_guard) = player.write() {
            player_guard.heal_all_objects();
        }

        self.play_heal_audio(&other.get_position());
        Ok(true)
    }

    fn play_heal_audio(&self, position: &Coord3D) {
        if let Some(audio) = TheAudio::get() {
            let mut audio_event = TheAudio::get_misc_audio().crate_heal.clone();
            audio_event.set_position(&(position.x, position.y, position.z));
            audio.add_audio_event(&audio_event);
        }
    }
}

impl CrateCollideBehavior for HealCrateCollide {
    fn execute_crate_behavior(&mut self, other: &dyn GameObject) -> Result<bool, CollisionError> {
        self.execute_healing(other)
    }

    fn is_valid_to_execute(&self, other: &dyn GameObject) -> bool {
        self.base_crate.is_valid_to_execute(other)
    }
}

impl CollideModule for HealCrateCollide {
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        _loc: &Coord3D,
        _normal: &Coord3D,
    ) -> Result<(), CollisionError> {
        let Some(other_obj) = other else {
            return Ok(());
        };

        if !self.base_crate.is_valid_to_execute(other_obj) {
            return Ok(());
        }

        let success = self.execute_crate_behavior(other_obj)?;
        self.base_crate
            .finish_execution_attempt(other_obj, success)?;

        Ok(())
    }

    fn would_like_to_collide_with(&self, other: &dyn GameObject) -> bool {
        CrateCollideBehavior::is_valid_to_execute(self, other)
    }
}

/// Factory for creating HealCrateCollide modules.
pub struct HealCrateCollideFactory;

impl HealCrateCollideFactory {
    pub fn create(object_id: ObjectId) -> HealCrateCollide {
        let data = HealCrateCollideModuleData::new();
        HealCrateCollide::new(object_id, data)
    }

    pub fn create_with_config(
        object_id: ObjectId,
        config: HealCrateCollideModuleData,
    ) -> HealCrateCollide {
        HealCrateCollide::new(object_id, config)
    }
}

impl game_engine::common::system::Snapshotable for HealCrateCollide {
    fn crc(&self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        self.base_crate.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| err.to_string())?;
        self.base_crate.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base_crate.load_post_process()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::rts::science::{
        get_science_store, get_science_store_mut, init_science_store, ScienceInfo, SCIENCE_INVALID,
    };

    fn install_test_science(name: &str) -> crate::common::science::ScienceType {
        init_science_store();
        {
            let mut store = get_science_store_mut().expect("science store mut");
            store.init();
            store.add_science(ScienceInfo::new(SCIENCE_INVALID, name));
        }
        get_science_store()
            .expect("science store")
            .get_science_from_internal_name(name) as crate::common::science::ScienceType
    }

    #[test]
    fn heal_crate_parse_from_ini_preserves_cpp_base_fields() {
        let _lock = crate::test_sync::lock();

        let expected_science = install_test_science("SCIENCE_HEAL_CRATE_TEST");
        let mut data = HealCrateCollideModuleData::default();
        let mut ini = INI::new();
        ini.with_inline_source(
            "RequiredKindOf = VEHICLE|INFANTRY\n\
             ForbiddenKindOf = DRONE\n\
             ForbidOwnerPlayer = true\n\
             BuildingPickup = false\n\
             HumanOnly = true\n\
             PickupScience = SCIENCE_HEAL_CRATE_TEST\n\
             ExecuteFX = FX_CratePickup\n\
             ExecuteAnimation = HealCrateAnim\n\
             ExecuteAnimationTime = 1.75\n\
             ExecuteAnimationZRise = 2.5\n\
             ExecuteAnimationFades = false\n\
             End\n",
            |ini| data.parse_from_ini(ini),
        )
        .expect("heal crate ini parses");

        assert_ne!(
            data.base.required_kind_of & (1u64 << (KindOf::Vehicle as u32)),
            0
        );
        assert_ne!(
            data.base.required_kind_of & (1u64 << (KindOf::Infantry as u32)),
            0
        );
        assert_ne!(
            data.base.forbidden_kind_of & (1u64 << (KindOf::Drone as u32)),
            0
        );
        assert!(data.base.is_forbid_owner_player);
        assert!(!data.base.is_building_pickup);
        assert!(data.base.is_human_only_pickup);
        assert_eq!(data.base.pickup_science, expected_science);
        assert_eq!(data.base.execute_fx.as_deref(), Some("FX_CratePickup"));
        assert_eq!(data.base.execution_animation_template, "HealCrateAnim");
        assert!((data.base.execute_animation_display_time_seconds - 1.75).abs() < f32::EPSILON);
        assert!((data.base.execute_animation_z_rise_per_second - 2.5).abs() < f32::EPSILON);
        assert!(!data.base.execute_animation_fades);
    }

    #[test]
    fn heal_crate_rejects_missing_cpp_base_field_value() {
        let mut data = HealCrateCollideModuleData::default();
        let mut ini = INI::new();

        let err = ini
            .with_inline_source("RequiredKindOf =\nEnd\n", |ini| data.parse_from_ini(ini))
            .expect_err("missing kindof value should fail");

        assert!(matches!(err, INIError::InvalidData));
        assert_eq!(data.base.required_kind_of, 0);
    }

    #[test]
    fn heal_crate_rejects_unknown_pickup_science_like_cpp() {
        let _lock = crate::test_sync::lock();

        install_test_science("SCIENCE_KNOWN_HEAL_CRATE_TEST");

        let mut data = HealCrateCollideModuleData::default();
        let mut ini = INI::new();
        let err = ini
            .with_inline_source(
                "PickupScience = SCIENCE_DOES_NOT_EXIST\n\
                 End\n",
                |ini| data.parse_from_ini(ini),
            )
            .expect_err("unknown PickupScience should fail");

        assert!(matches!(err, INIError::InvalidData));
        assert_eq!(data.base.pickup_science, SCIENCE_INVALID);
    }
}
