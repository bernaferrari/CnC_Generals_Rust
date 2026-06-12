//! Shroud Crate Collision Module
//!
//! FILE: shroud_crate_collide.rs
//! Author: Converted from Graham Smallwood's C++ implementation, March 2002
//! Desc: A crate that clears the shroud for the picker-upper

use super::*;
use crate::common::{kindof_from_name, LegacyModuleData};
use crate::helpers::TheAudio;
use crate::object::collide::crate_collide::crate_collide::CrateCollide as LegacyCrateCollide;
use crate::object::collide::*;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{ModuleData, NameKeyType, ShroudCrateCollideConfig};

/// Module data for shroud crates. C++ exposes only the inherited CrateCollide
/// fields, so the shroud-specific config is the base crate collision data.
#[derive(Debug, Clone)]
pub struct ShroudCrateCollideModuleData {
    module_tag_name_key: NameKeyType,
    crate_data: CrateCollideModuleData,
}

impl Default for ShroudCrateCollideModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            crate_data: CrateCollideModuleData::default(),
        }
    }
}

impl ShroudCrateCollideModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SHROUD_CRATE_COLLIDE_FIELDS)
    }

    pub fn crate_data(&self) -> CrateCollideModuleData {
        self.crate_data.clone()
    }

    fn to_config(&self) -> ShroudCrateCollideConfig {
        ShroudCrateCollideConfig {
            required_kind_of: self.crate_data.required_kind_of,
            forbidden_kind_of: self.crate_data.forbidden_kind_of,
            is_forbid_owner_player: self.crate_data.is_forbid_owner_player,
            is_building_pickup: self.crate_data.is_building_pickup,
            is_human_only_pickup: self.crate_data.is_human_only_pickup,
            pickup_science: self.crate_data.pickup_science,
            execute_fx: self.crate_data.execute_fx.clone(),
            execution_animation_template: self.crate_data.execution_animation_template.clone(),
            execute_animation_display_time_seconds: self
                .crate_data
                .execute_animation_display_time_seconds,
            execute_animation_z_rise_per_second: self
                .crate_data
                .execute_animation_z_rise_per_second,
            execute_animation_fades: self.crate_data.execute_animation_fades,
        }
    }

    pub fn from_config(config: ShroudCrateCollideConfig, module_tag_name_key: NameKeyType) -> Self {
        let mut crate_data = CrateCollideModuleData::default();
        crate_data.required_kind_of = config.required_kind_of;
        crate_data.forbidden_kind_of = config.forbidden_kind_of;
        crate_data.is_forbid_owner_player = config.is_forbid_owner_player;
        crate_data.is_building_pickup = config.is_building_pickup;
        crate_data.is_human_only_pickup = config.is_human_only_pickup;
        crate_data.pickup_science = config.pickup_science;
        crate_data.execute_fx = config.execute_fx;
        crate_data.execution_animation_template = config.execution_animation_template;
        crate_data.execute_animation_display_time_seconds =
            config.execute_animation_display_time_seconds;
        crate_data.execute_animation_z_rise_per_second = config.execute_animation_z_rise_per_second;
        crate_data.execute_animation_fades = config.execute_animation_fades;

        Self {
            module_tag_name_key,
            crate_data,
        }
    }
}

impl LegacyModuleData for ShroudCrateCollideModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }

    fn get_shroud_crate_collide_config(&self) -> Option<ShroudCrateCollideConfig> {
        Some(self.to_config())
    }
}

impl ModuleData for ShroudCrateCollideModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }

    fn get_shroud_crate_collide_config(&self) -> Option<ShroudCrateCollideConfig> {
        Some(self.to_config())
    }
}

impl crate::common::types::ModuleData for ShroudCrateCollideModuleData {
    fn get_shroud_crate_collide_config(&self) -> Option<ShroudCrateCollideConfig> {
        Some(self.to_config())
    }
}

impl Snapshotable for ShroudCrateCollideModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("ShroudCrateCollideModuleData xfer version: {e:?}"))
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Shroud Crate Collide Module
///
/// This module implements a crate that reveals the entire map (clears shroud)
/// for the player who picks it up.
pub struct ShroudCrateCollide {
    base: LegacyCrateCollide,
    version: u32,
}

impl ShroudCrateCollide {
    /// Create a new ShroudCrateCollide instance
    ///
    /// # Arguments
    /// * `object_id` - The ID of the object this module belongs to
    /// * `module_data` - Configuration data for the crate collision behavior
    pub fn new(object_id: ObjectId, module_data: CrateCollideModuleData) -> Self {
        Self {
            base: LegacyCrateCollide::new(object_id, module_data),
            version: 1,
        }
    }

    /// Get the current version of this module for serialization
    pub fn get_version(&self) -> u32 {
        self.version
    }
}

impl CollideModule for ShroudCrateCollide {
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        _loc: &Coord3D,
        _normal: &Coord3D,
    ) -> Result<(), CollisionError> {
        if let Some(other_obj) = other {
            if self.base.is_valid_to_execute(other_obj) {
                // Execute the shroud crate behavior
                let success = self.execute_crate_behavior_internal(other_obj)?;
                self.base.finish_execution_attempt(other_obj, success)?;
            }
        }

        Ok(())
    }

    fn would_like_to_collide_with(&self, other: &dyn GameObject) -> bool {
        self.base.is_valid_to_execute(other)
    }
}

impl ShroudCrateCollide {
    /// Internal implementation of crate behavior execution
    ///
    /// This method reveals the entire map for the controlling player of the object
    /// that collided with this crate, and plays a crate pickup sound.
    ///
    /// # Arguments
    /// * `other` - The object that collided with this crate
    ///
    /// # Returns
    /// * `Ok(true)` if the crate behavior was successfully executed
    /// * `Ok(false)` if the behavior could not be executed
    /// * `Err(CollisionError)` if an error occurred during execution
    fn execute_crate_behavior_internal(
        &self,
        other: &dyn GameObject,
    ) -> Result<bool, CollisionError> {
        // Get the controlling player of the object that picked up the crate
        let crate_player = other.get_controlling_player();
        let player_id = crate_player.value() as u32;

        // Reveal the entire map for this player
        if let Ok(mut shroud_manager) = crate::system::shroud_manager::get_shroud_manager().lock() {
            let _ = shroud_manager.reveal_map_for_player(player_id);
        }

        // C++ parity: use MiscAudio::m_crateShroud and bind the event to the picker object ID.
        if let Some(audio) = TheAudio::get() {
            let mut event = TheAudio::get_misc_audio().crate_shroud.clone();
            event.object_id = other.get_id();
            audio.add_misc_audio_event(&event);
        }

        Ok(true)
    }
}

// Mock-based tests removed to avoid mocks in fidelity-critical code.

impl game_engine::common::system::Snapshotable for ShroudCrateCollide {
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

fn parse_kind_of_mask(tokens: &[&str]) -> Result<u64, INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }

    let mut mask = 0u64;
    for token in tokens.iter().flat_map(|token| token.split('|')) {
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
    data: &mut ShroudCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.crate_data.required_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbidden_kind_of(
    _ini: &mut INI,
    data: &mut ShroudCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.crate_data.forbidden_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbid_owner_player(
    _ini: &mut INI,
    data: &mut ShroudCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.crate_data.is_forbid_owner_player = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_building_pickup(
    _ini: &mut INI,
    data: &mut ShroudCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.crate_data.is_building_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_human_only(
    _ini: &mut INI,
    data: &mut ShroudCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.crate_data.is_human_only_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_pickup_science(
    _ini: &mut INI,
    data: &mut ShroudCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.crate_data.pickup_science =
        NameKeyGenerator::name_to_key(first_token(tokens)?) as crate::common::science::ScienceType;
    Ok(())
}

fn parse_execute_fx(
    _ini: &mut INI,
    data: &mut ShroudCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.crate_data.execute_fx = Some(first_token(tokens)?.to_string());
    Ok(())
}

fn parse_execute_animation(
    _ini: &mut INI,
    data: &mut ShroudCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.crate_data.execution_animation_template = first_token(tokens)?.to_string();
    Ok(())
}

fn parse_execute_animation_time(
    _ini: &mut INI,
    data: &mut ShroudCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.crate_data.execute_animation_display_time_seconds = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_z_rise(
    _ini: &mut INI,
    data: &mut ShroudCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.crate_data.execute_animation_z_rise_per_second = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_fades(
    _ini: &mut INI,
    data: &mut ShroudCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.crate_data.execute_animation_fades = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn first_token<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens.first().copied().ok_or(INIError::InvalidData)
}

const SHROUD_CRATE_COLLIDE_FIELDS: &[FieldParse<ShroudCrateCollideModuleData>] = &[
    FieldParse {
        token: "RequiredKindOf",
        parse: parse_required_kind_of,
    },
    FieldParse {
        token: "ForbiddenKindOf",
        parse: parse_forbidden_kind_of,
    },
    FieldParse {
        token: "ForbidOwnerPlayer",
        parse: parse_forbid_owner_player,
    },
    FieldParse {
        token: "BuildingPickup",
        parse: parse_building_pickup,
    },
    FieldParse {
        token: "HumanOnly",
        parse: parse_human_only,
    },
    FieldParse {
        token: "PickupScience",
        parse: parse_pickup_science,
    },
    FieldParse {
        token: "ExecuteFX",
        parse: parse_execute_fx,
    },
    FieldParse {
        token: "ExecuteAnimation",
        parse: parse_execute_animation,
    },
    FieldParse {
        token: "ExecuteAnimationTime",
        parse: parse_execute_animation_time,
    },
    FieldParse {
        token: "ExecuteAnimationZRise",
        parse: parse_execute_animation_z_rise,
    },
    FieldParse {
        token: "ExecuteAnimationFades",
        parse: parse_execute_animation_fades,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::KindOf;

    #[test]
    fn shroud_crate_config_preserves_base_crate_fields() {
        let mut data = ShroudCrateCollideModuleData::default();
        LegacyModuleData::set_module_tag_name_key(&mut data, 0xCAFE);
        data.crate_data.required_kind_of = 1u64 << (KindOf::Vehicle as u32);
        data.crate_data.forbidden_kind_of = 1u64 << (KindOf::Aircraft as u32);
        data.crate_data.is_forbid_owner_player = true;
        data.crate_data.is_building_pickup = true;
        data.crate_data.is_human_only_pickup = true;
        data.crate_data.pickup_science = 17;
        data.crate_data.execute_fx = Some("FX_Test".to_string());
        data.crate_data.execution_animation_template = "Anim_Test".to_string();
        data.crate_data.execute_animation_display_time_seconds = 3.5;
        data.crate_data.execute_animation_z_rise_per_second = 2.0;
        data.crate_data.execute_animation_fades = false;

        let rebuilt = ShroudCrateCollideModuleData::from_config(data.to_config(), 0xBEEF);

        assert_eq!(LegacyModuleData::get_module_tag_name_key(&rebuilt), 0xBEEF);
        assert_eq!(
            rebuilt.crate_data.required_kind_of,
            data.crate_data.required_kind_of
        );
        assert_eq!(
            rebuilt.crate_data.forbidden_kind_of,
            data.crate_data.forbidden_kind_of
        );
        assert!(rebuilt.crate_data.is_forbid_owner_player);
        assert!(rebuilt.crate_data.is_building_pickup);
        assert!(rebuilt.crate_data.is_human_only_pickup);
        assert_eq!(rebuilt.crate_data.pickup_science, 17);
        assert_eq!(rebuilt.crate_data.execute_fx.as_deref(), Some("FX_Test"));
        assert_eq!(rebuilt.crate_data.execution_animation_template, "Anim_Test");
        assert_eq!(
            rebuilt.crate_data.execute_animation_display_time_seconds,
            3.5
        );
        assert_eq!(rebuilt.crate_data.execute_animation_z_rise_per_second, 2.0);
        assert!(!rebuilt.crate_data.execute_animation_fades);
    }

    #[test]
    fn shroud_crate_kindof_parser_accepts_pipe_and_space_tokens() {
        let mask = parse_kind_of_mask(&["VEHICLE|INFANTRY", "STRUCTURE"]).expect("kind mask");

        assert_ne!(mask & (1u64 << (KindOf::Vehicle as u32)), 0);
        assert_ne!(mask & (1u64 << (KindOf::Infantry as u32)), 0);
        assert_ne!(mask & (1u64 << (KindOf::Structure as u32)), 0);
    }
}
