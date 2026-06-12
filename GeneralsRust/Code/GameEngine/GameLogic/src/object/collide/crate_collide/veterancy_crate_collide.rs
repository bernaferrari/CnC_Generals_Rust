//! Veterancy Crate Collision Module
//!
//! FILE: veterancy_crate_collide.rs
//! Author: Converted from Graham Smallwood's C++ implementation, March 2002
//! Desc: A crate that gives a level of experience to all within n distance

use super::*;
use crate::common::{kindof_from_name, FieldParse, FieldType, KindOf};
use crate::experience::ExperienceRequirements;
use crate::helpers::TheGameLogic;
use crate::object::collide::crate_collide::crate_collide::CrateCollide as LegacyCrateCollide;
use crate::object::collide::partition_filters::{
    PartitionFilterSameMapStatus, PartitionFilterSamePlayer,
};
use crate::object::collide::*;
use crate::object::registry::OBJECT_REGISTRY;
use crate::scripting::engine::transfer_object_name;
use game_engine::common::ini::{FieldParse as IniFieldParse, INIError, INI};
use std::sync::{Arc, Mutex};

/// Module data specific to veterancy crate collision
#[derive(Debug, Clone)]
pub struct VeterancyCrateCollideModuleData {
    pub base: CrateCollideModuleData,
    /// Range of effect for veterancy bonus (0 = single target only)
    pub range_of_effect: u32,
    /// If true, adds owner's veterancy level to bonus
    pub adds_owner_veterancy: bool,
    /// If true, this is a pilot entering a vehicle
    pub is_pilot: bool,
}

impl Default for VeterancyCrateCollideModuleData {
    fn default() -> Self {
        Self {
            base: CrateCollideModuleData::default(),
            range_of_effect: 0,
            adds_owner_veterancy: false,
            is_pilot: false,
        }
    }
}

impl VeterancyCrateCollideModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, VETERANCY_CRATE_COLLIDE_FIELDS)
    }

    pub fn build_field_parse() -> Vec<FieldParse> {
        let mut fields = CrateCollideModuleData::build_field_parse();
        fields.extend([
            FieldParse::new("EffectRange", FieldType::UnsignedInt, "range_of_effect"),
            FieldParse::new("AddsOwnerVeterancy", FieldType::Int, "adds_owner_veterancy"),
            FieldParse::new("IsPilot", FieldType::Int, "is_pilot"),
        ]);
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
    data: &mut VeterancyCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.required_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbidden_kind_of(
    _ini: &mut INI,
    data: &mut VeterancyCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.forbidden_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbid_owner_player(
    _ini: &mut INI,
    data: &mut VeterancyCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_forbid_owner_player = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_building_pickup(
    _ini: &mut INI,
    data: &mut VeterancyCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_building_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_human_only(
    _ini: &mut INI,
    data: &mut VeterancyCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_human_only_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_pickup_science(
    _ini: &mut INI,
    data: &mut VeterancyCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    super::parse_crate_pickup_science(&mut data.base, first_token(tokens)?)
}

fn parse_execute_fx(
    _ini: &mut INI,
    data: &mut VeterancyCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_fx = Some(first_token(tokens)?.to_string());
    Ok(())
}

fn parse_execute_animation(
    _ini: &mut INI,
    data: &mut VeterancyCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execution_animation_template = first_token(tokens)?.to_string();
    Ok(())
}

fn parse_execute_animation_time(
    _ini: &mut INI,
    data: &mut VeterancyCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_display_time_seconds = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_z_rise(
    _ini: &mut INI,
    data: &mut VeterancyCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_z_rise_per_second = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_fades(
    _ini: &mut INI,
    data: &mut VeterancyCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_fades = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_effect_range(
    _ini: &mut INI,
    data: &mut VeterancyCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.range_of_effect = INI::parse_unsigned_int(first_token(tokens)?)?;
    Ok(())
}

fn parse_adds_owner_veterancy(
    _ini: &mut INI,
    data: &mut VeterancyCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.adds_owner_veterancy = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_is_pilot(
    _ini: &mut INI,
    data: &mut VeterancyCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.is_pilot = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

const VETERANCY_CRATE_COLLIDE_FIELDS: &[IniFieldParse<VeterancyCrateCollideModuleData>] = &[
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
        token: "EffectRange",
        parse: parse_effect_range,
    },
    IniFieldParse {
        token: "AddsOwnerVeterancy",
        parse: parse_adds_owner_veterancy,
    },
    IniFieldParse {
        token: "IsPilot",
        parse: parse_is_pilot,
    },
];

/// Veterancy Crate Collide Module
///
/// This module implements a crate that grants veterancy experience to units.
/// It can affect a single unit or all units within a specified range.
pub struct VeterancyCrateCollide {
    base: LegacyCrateCollide,
    module_data: VeterancyCrateCollideModuleData,
    owner_object_id: ObjectId,
    version: u32,
}

impl VeterancyCrateCollide {
    /// Create a new VeterancyCrateCollide instance
    ///
    /// # Arguments
    /// * `object_id` - The ID of the object this module belongs to
    /// * `module_data` - Configuration data for the veterancy crate collision behavior
    pub fn new(object_id: ObjectId, module_data: VeterancyCrateCollideModuleData) -> Self {
        Self {
            base: LegacyCrateCollide::new(object_id, module_data.base.clone()),
            module_data,
            owner_object_id: object_id,
            version: 1,
        }
    }

    /// Get the veterancy crate collision module data
    pub fn get_veterancy_crate_collide_module_data(&self) -> &VeterancyCrateCollideModuleData {
        &self.module_data
    }

    /// Get the number of levels this crate will grant
    pub fn get_levels_to_gain(&self) -> i32 {
        if !self.module_data.adds_owner_veterancy {
            return 1;
        }

        // C++ parity: derive levels from the crate owner's veterancy.
        let Some(owner_obj) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return 0;
        };
        let Ok(owner_guard) = owner_obj.read() else {
            return 0;
        };
        owner_guard.get_veterancy_level() as i32
    }

    /// Get the current version of this module for serialization
    pub fn get_version(&self) -> u32 {
        self.version
    }
}

impl CollideModule for VeterancyCrateCollide {
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        _loc: &Coord3D,
        _normal: &Coord3D,
    ) -> Result<(), CollisionError> {
        if let Some(other_obj) = other {
            if self.is_valid_to_execute_internal(other_obj) {
                // Execute the veterancy crate behavior
                let success = self.execute_crate_behavior_internal(other_obj)?;
                self.base.finish_execution_attempt(other_obj, success)?;
            }
        }

        Ok(())
    }

    fn would_like_to_collide_with(&self, other: &dyn GameObject) -> bool {
        self.is_valid_to_execute_internal(other)
    }
}

impl VeterancyCrateCollide {
    fn owner_goal_matches(&self, target_id: ObjectId) -> bool {
        let Some(owner_obj) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return false;
        };
        let ai_update = match owner_obj.read() {
            Ok(owner_guard) => owner_guard.get_ai_update_interface(),
            Err(_) => None,
        };
        let Some(ai_update) = ai_update else {
            return false;
        };
        // Avoid deadlocking crate collection when the AI update/machine lock is currently held.
        let Ok(ai_guard) = ai_update.try_lock() else {
            return false;
        };
        let goal_id = ai_guard
            .get_goal_object()
            .and_then(|goal| goal.read().ok().map(|goal_guard| goal_guard.get_id()));
        goal_id == Some(target_id)
    }

    fn owner_player_id(&self) -> Option<PlayerId> {
        let owner_obj = TheGameLogic::find_object_by_id(self.owner_object_id)?;
        let owner_guard = owner_obj.read().ok()?;
        owner_guard.get_player_id()
    }

    /// Enhanced validation for veterancy crate execution
    ///
    /// This method checks if the crate can be executed for the given object,
    /// including special checks for pilots and aircraft.
    fn is_valid_to_execute_internal(&self, other: &dyn GameObject) -> bool {
        // Base validation first
        if !self.base.is_valid_to_execute(other) {
            return false;
        }

        if other.is_effectively_dead() {
            return false;
        }

        if other.is_significantly_above_terrain() {
            return false;
        }

        let levels_to_gain = self.get_levels_to_gain();

        if levels_to_gain <= 0 {
            return false;
        }

        let Some(other_handle) = other.as_object_handle() else {
            return false;
        };
        let Ok(other_guard) = other_handle.read() else {
            return false;
        };
        let Some(tracker) = other_guard.get_experience_tracker() else {
            return false;
        };
        let Ok(tracker_guard) = tracker.lock() else {
            return false;
        };
        if !tracker_guard.is_trainable() || !tracker_guard.can_gain_exp_for_level(levels_to_gain) {
            return false;
        }

        // Pilot-specific checks
        if self.module_data.is_pilot {
            if self.owner_player_id() != Some(other.get_controlling_player()) {
                return false;
            }

            // Can't upgrade a helicopter or plane
            if other.is_using_airborne_locomotor() {
                return false;
            }
        }

        true
    }

    /// Internal implementation of crate behavior execution
    ///
    /// This method grants veterancy experience to the target object or all objects
    /// within the specified range, depending on the module configuration.
    fn collect_area_effect_object_ids(&self, other: &dyn GameObject, range: f32) -> Vec<ObjectId> {
        let center = other.get_position();
        let range_sqr = range * range;
        let same_player_filter = PartitionFilterSamePlayer::new(other.get_controlling_player());
        let same_map_status_filter = PartitionFilterSameMapStatus::new(other.get_id());

        OBJECT_REGISTRY
            .get_all_objects()
            .into_iter()
            .filter_map(|obj_arc| {
                let obj_position = obj_arc.get_position();
                let dx = obj_position.x - center.x;
                let dy = obj_position.y - center.y;
                if dx * dx + dy * dy > range_sqr {
                    return None;
                }
                if !same_player_filter.allow(&obj_arc) || !same_map_status_filter.allow(&obj_arc) {
                    return None;
                }
                obj_arc.read().ok().and_then(|obj_guard| {
                    if obj_guard.is_destroyed() {
                        None
                    } else {
                        Some(obj_guard.get_id())
                    }
                })
            })
            .collect()
    }

    fn execute_crate_behavior_internal(
        &self,
        other: &dyn GameObject,
    ) -> Result<bool, CollisionError> {
        // C++ parity: crate owner AI goal must intentionally target `other`.
        if !self.owner_goal_matches(other.get_id()) {
            return Ok(false);
        }

        let levels_to_gain = self.get_levels_to_gain();
        let range = self.module_data.range_of_effect as f32;

        let mut affected_objects = Vec::new();

        if range == 0.0 {
            affected_objects.push(other.get_id());
        } else {
            affected_objects.extend(self.collect_area_effect_object_ids(other, range));
        }

        let requirements = ExperienceRequirements::default_requirements();
        for object_id in affected_objects {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(mut obj_guard) = obj_arc.write() else {
                continue;
            };
            let Some(tracker) = obj_guard.get_experience_tracker() else {
                continue;
            };
            let Ok(mut tracker_guard) = tracker.lock() else {
                continue;
            };
            if !tracker_guard.can_gain_exp_for_level(levels_to_gain) {
                continue;
            }
            let old_level = tracker_guard.get_veterancy_level();
            if tracker_guard.gain_exp_for_level(
                levels_to_gain,
                !self.module_data.is_pilot,
                requirements.as_array(),
            ) {
                let new_level = tracker_guard.get_veterancy_level();
                if old_level != new_level {
                    obj_guard.on_veterancy_level_changed(old_level, new_level, true);
                }
            }
        }

        // Transfer object name for pilots (for script control)
        if self.module_data.is_pilot {
            let owner_name = TheGameLogic::find_object_by_id(self.owner_object_id)
                .and_then(|obj| obj.read().ok().map(|obj| obj.get_name().clone()))
                .unwrap_or_else(|| format!("Object{}", self.owner_object_id).into());
            transfer_object_name(&owner_name, other.get_id())
                .map_err(|e| CollisionError::InvalidObject(e.to_string()))?;
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::TheThingFactory;
    use crate::object_manager::get_object_manager;
    use crate::player::{player_list, Player, PlayerIndex};
    use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
    use std::sync::{Arc, RwLock};

    fn ensure_template_exists(name: &str) {
        let needs_init = get_thing_factory().unwrap().is_none();
        if needs_init {
            init_thing_factory().unwrap();
        }
        let mut factory_guard = get_thing_factory().unwrap();
        if let Some(factory) = factory_guard.as_mut() {
            if factory.find_template(name, false).is_none() {
                factory.new_template(name);
            }
        }
    }

    fn setup_player_with_team(
        player_index: PlayerIndex,
        team_name: &str,
    ) -> Arc<RwLock<crate::team::Team>> {
        {
            let player_list = player_list();
            let mut list_guard = player_list.write().expect("Player list lock poisoned");
            list_guard.clear();
        }

        let team_arc = Arc::new(RwLock::new(crate::team::Team::new(
            crate::common::AsciiString::from(team_name),
            (player_index as u32).saturating_add(1),
        )));

        if let Ok(mut team_guard) = team_arc.write() {
            team_guard.set_controlling_player_id(Some(player_index as u32));
        }

        let player_arc = Arc::new(RwLock::new(Player::new(player_index)));
        if let Ok(mut player_guard) = player_arc.write() {
            player_guard.set_default_team(Some(Arc::clone(&team_arc)));
        }
        {
            let player_list = player_list();
            let mut list_guard = player_list.write().expect("Player list lock poisoned");
            list_guard.add_player(player_arc);
        }

        team_arc
    }

    fn create_object_with_team(
        template_name: &str,
        team_arc: &Arc<RwLock<crate::team::Team>>,
        position: Coord3D,
    ) -> Arc<RwLock<crate::object::Object>> {
        ensure_template_exists(template_name);
        let team_guard = team_arc.read().expect("Team lock poisoned");

        let thing_factory = TheThingFactory::get().expect("ThingFactory unavailable");
        let template = TheThingFactory::find_template(template_name).expect("Template missing");
        let obj = thing_factory
            .new_object(template, &*team_guard)
            .expect("Failed to create object");

        if let Ok(mut obj_guard) = obj.write() {
            let object_position = crate::common::Coord3D::new(position.x, position.y, position.z);
            let _ = obj_guard.set_position(&object_position);
        }

        if let Ok(mut manager) = get_object_manager().write() {
            let object_id = obj.read().map(|o| o.get_id()).unwrap_or(0);
            let object_position = crate::common::Coord3D::new(position.x, position.y, position.z);
            manager.update_object_position(object_id, object_position);
        }

        obj
    }

    fn create_registered_test_object(
        id: ObjectId,
        team_arc: &Arc<RwLock<crate::team::Team>>,
        position: Coord3D,
    ) -> Arc<RwLock<crate::object::Object>> {
        let object = Arc::new(RwLock::new(crate::object::Object::new_test(id, 100.0)));
        {
            let mut guard = object.write().expect("object write");
            let object_position = crate::common::Coord3D::new(position.x, position.y, position.z);
            guard
                .set_position(&object_position)
                .expect("object position");
            guard.set_height_above_terrain(position.z);
            guard
                .set_team(Some(Arc::clone(team_arc)))
                .expect("object team");
        }
        OBJECT_REGISTRY.register_object(id, &object);
        object
    }

    #[test]
    fn veterancy_crate_parse_from_ini_preserves_cpp_fields() {
        let _lock = crate::test_sync::lock();

        let mut data = VeterancyCrateCollideModuleData::default();
        let mut ini = INI::new();
        ini.with_inline_source(
            "EffectRange = 225\n\
             AddsOwnerVeterancy = Yes\n\
             IsPilot = true\n\
             RequiredKindOf = INFANTRY|VEHICLE\n\
             ExecuteAnimationTime = 1.75\n\
             End\n",
            |ini| data.parse_from_ini(ini),
        )
        .expect("veterancy crate ini parses");

        assert_eq!(data.range_of_effect, 225);
        assert!(data.adds_owner_veterancy);
        assert!(data.is_pilot);
        assert_ne!(
            data.base.required_kind_of & (1u64 << (KindOf::Infantry as u32)),
            0
        );
        assert_ne!(
            data.base.required_kind_of & (1u64 << (KindOf::Vehicle as u32)),
            0
        );
        assert!((data.base.execute_animation_display_time_seconds - 1.75).abs() < f32::EPSILON);
    }

    #[test]
    fn veterancy_crate_rejects_missing_cpp_field_value() {
        let mut data = VeterancyCrateCollideModuleData::default();
        let mut ini = INI::new();

        let err = ini
            .with_inline_source("EffectRange =\nEnd\n", |ini| data.parse_from_ini(ini))
            .expect_err("missing effect range should fail");

        assert!(matches!(err, INIError::InvalidData));
        assert_eq!(data.range_of_effect, 0);
    }

    #[test]
    fn veterancy_crate_build_field_parse_exposes_cpp_tokens() {
        let fields = VeterancyCrateCollideModuleData::build_field_parse();
        assert!(fields
            .iter()
            .any(|field| field.token == "EffectRange" && field.target == "range_of_effect"));
        assert!(fields
            .iter()
            .any(|field| field.token == "AddsOwnerVeterancy"
                && field.target == "adds_owner_veterancy"));
        assert!(fields
            .iter()
            .any(|field| field.token == "IsPilot" && field.target == "is_pilot"));
    }

    #[test]
    fn test_veterancy_crate_creation() {
        let _lock = crate::test_sync::lock();

        let module_data = VeterancyCrateCollideModuleData {
            range_of_effect: 10,
            adds_owner_veterancy: true,
            is_pilot: false,
            ..Default::default()
        };

        let veterancy_crate = VeterancyCrateCollide::new(1, module_data);

        assert_eq!(veterancy_crate.get_version(), 1);
        assert_eq!(
            veterancy_crate
                .get_veterancy_crate_collide_module_data()
                .range_of_effect,
            10
        );
        assert!(
            veterancy_crate
                .get_veterancy_crate_collide_module_data()
                .adds_owner_veterancy
        );
    }

    #[test]
    fn test_veterancy_crate_levels_to_gain() {
        let _lock = crate::test_sync::lock();

        let module_data = VeterancyCrateCollideModuleData {
            adds_owner_veterancy: true,
            ..Default::default()
        };

        let veterancy_crate = VeterancyCrateCollide::new(1, module_data);

        // No owner object exists in this unit test setup.
        assert_eq!(veterancy_crate.get_levels_to_gain(), 0);

        let module_data_no_owner = VeterancyCrateCollideModuleData {
            adds_owner_veterancy: false,
            ..Default::default()
        };

        let veterancy_crate_no_owner = VeterancyCrateCollide::new(1, module_data_no_owner);
        assert_eq!(veterancy_crate_no_owner.get_levels_to_gain(), 1);
    }

    #[test]
    fn veterancy_area_effect_uses_same_map_status_not_height_like_cpp() {
        let _lock = crate::test_sync::lock();

        OBJECT_REGISTRY.clear();

        let team_arc = setup_player_with_team(1, "VeterancyAreaTeam");
        let map_extent = crate::helpers::TheTerrainLogic::get()
            .expect("terrain logic")
            .get_maximum_pathfind_extent();
        let edge_x = map_extent.hi.x;
        let y = map_extent.lo.y + 10.0;

        let other =
            create_registered_test_object(71_000, &team_arc, Coord3D::new(edge_x - 10.0, y, 0.0));
        let elevated_on_map =
            create_registered_test_object(71_001, &team_arc, Coord3D::new(edge_x - 5.0, y, 100.0));
        let off_map_nearby =
            create_registered_test_object(71_002, &team_arc, Coord3D::new(edge_x + 10.0, y, 0.0));

        assert!(!other.is_significantly_above_terrain());
        assert!(elevated_on_map.is_significantly_above_terrain());
        assert!(!other.read().expect("other read").is_off_map());
        assert!(!elevated_on_map.read().expect("elevated read").is_off_map());
        assert!(off_map_nearby.read().expect("off map read").is_off_map());

        let veterancy_crate =
            VeterancyCrateCollide::new(71_100, VeterancyCrateCollideModuleData::default());
        let affected = veterancy_crate.collect_area_effect_object_ids(&other, 50.0);

        assert!(affected.contains(&71_000));
        assert!(affected.contains(&71_001));
        assert!(!affected.contains(&71_002));

        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn test_veterancy_crate_execute_behavior() {
        let _lock = crate::test_sync::lock();

        let team_arc = setup_player_with_team(1, "PlayerTeam");
        let other = create_object_with_team("Infantry", &team_arc, Coord3D::new(10.0, 20.0, 0.0));

        let module_data = VeterancyCrateCollideModuleData {
            range_of_effect: 0, // Single target
            ..Default::default()
        };

        let veterancy_crate = VeterancyCrateCollide::new(1, module_data);

        let result = veterancy_crate.execute_crate_behavior_internal(&other);
        assert!(result.is_ok());
        // C++ parity guard: crate owner AI goal must explicitly match target object.
        assert_eq!(result.unwrap(), false);
    }

    #[test]
    fn test_veterancy_crate_area_effect() {
        let _lock = crate::test_sync::lock();

        let team_arc = setup_player_with_team(1, "PlayerTeam");
        let other = create_object_with_team("Infantry", &team_arc, Coord3D::new(10.0, 20.0, 0.0));
        let _friend = create_object_with_team("Infantry", &team_arc, Coord3D::new(15.0, 20.0, 0.0));

        let module_data = VeterancyCrateCollideModuleData {
            range_of_effect: 15, // Area effect
            ..Default::default()
        };

        let veterancy_crate = VeterancyCrateCollide::new(1, module_data);

        let result = veterancy_crate.execute_crate_behavior_internal(&other);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false);
    }
}

impl game_engine::common::system::Snapshotable for VeterancyCrateCollide {
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
