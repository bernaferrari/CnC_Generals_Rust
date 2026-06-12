//! Unit Crate Collision Module
//!
//! FILE: unit_crate_collide.rs
//! Author: Converted from Graham Smallwood's C++ implementation, March 2002
//! Desc: A crate that gives n units of type m to the picker-upper

use super::*;
use crate::common::kindof_from_name;
use crate::helpers::{FindPositionOptions, TheAudio, ThePartitionManager, TheThingFactory};
use crate::object::collide::crate_collide::crate_collide::CrateCollide as LegacyCrateCollide;
use crate::object::collide::*;
use crate::player::player_list;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::rts::science::{get_science_store, SCIENCE_INVALID};

/// Module data specific to unit crate collision
#[derive(Debug, Clone)]
pub struct UnitCrateCollideModuleData {
    pub base: CrateCollideModuleData,
    /// Number of units to create
    pub unit_count: u32,
    /// Type/name of unit to create
    pub unit_type: String,
}

impl Default for UnitCrateCollideModuleData {
    fn default() -> Self {
        Self {
            base: CrateCollideModuleData::default(),
            unit_count: 0,
            unit_type: String::new(),
        }
    }
}

impl UnitCrateCollideModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, UNIT_CRATE_COLLIDE_FIELDS)
    }
}

/// Unit Crate Collide Module
///
/// This module implements a crate that spawns a specified number of units
/// of a specified type for the player who picks it up.
pub struct UnitCrateCollide {
    base: LegacyCrateCollide,
    module_data: UnitCrateCollideModuleData,
    version: u32,
}

impl UnitCrateCollide {
    /// Create a new UnitCrateCollide instance
    ///
    /// # Arguments
    /// * `object_id` - The ID of the object this module belongs to
    /// * `module_data` - Configuration data for the unit crate collision behavior
    pub fn new(object_id: ObjectId, module_data: UnitCrateCollideModuleData) -> Self {
        Self {
            base: LegacyCrateCollide::new(object_id, module_data.base.clone()),
            module_data,
            version: 1,
        }
    }

    /// Get the unit crate collision module data
    pub fn get_unit_crate_collide_module_data(&self) -> &UnitCrateCollideModuleData {
        &self.module_data
    }

    /// Get the current version of this module for serialization
    pub fn get_version(&self) -> u32 {
        self.version
    }
}

impl CollideModule for UnitCrateCollide {
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        _loc: &Coord3D,
        _normal: &Coord3D,
    ) -> Result<(), CollisionError> {
        if let Some(other_obj) = other {
            if self.base.is_valid_to_execute(other_obj) {
                // Execute the unit crate behavior
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

impl UnitCrateCollide {
    /// Internal implementation of crate behavior execution
    ///
    /// This method creates the specified number of units of the specified type
    /// for the controlling player of the object that collided with this crate.
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
        let unit_count = self.module_data.unit_count;

        let thing_factory = TheThingFactory::get().map_err(|e| {
            CollisionError::InvalidObject(format!("Failed to acquire thing factory: {}", e))
        })?;

        let Some(unit_template) = TheThingFactory::find_template(&self.module_data.unit_type)
        else {
            return Ok(false);
        };

        let controlling_player = other.get_controlling_player();
        let team_arc = {
            let list_guard = player_list().read().map_err(|_| {
                CollisionError::InvalidObject("Player list lock poisoned".to_string())
            })?;
            let player_arc = list_guard
                .get_player(controlling_player.value() as i32)
                .cloned()
                .ok_or_else(|| {
                    CollisionError::InvalidObject(format!(
                        "Player {} not found for unit crate",
                        controlling_player.value()
                    ))
                })?;
            let player_guard = player_arc
                .read()
                .map_err(|_| CollisionError::InvalidObject("Player lock poisoned".to_string()))?;
            player_guard.get_default_team().ok_or_else(|| {
                CollisionError::InvalidObject(format!(
                    "Player {} has no default team",
                    controlling_player.value()
                ))
            })?
        };

        // Snapshot collision object transform once to avoid repeated object lock churn.
        let origin = other.get_position();
        let orientation = other.get_orientation();

        // Create the specified number of units
        for _unit_index in 0..unit_count {
            let new_obj = match thing_factory
                .new_object_with_team_handle(unit_template.clone(), team_arc.clone())
            {
                Ok(new_obj) => new_obj,
                Err(_) => continue,
            };

            // Set initial position and find a legal position around the crate
            let mut creation_point = crate::common::Coord3D::new(origin.x, origin.y, origin.z);
            if let Some(partition_manager) = ThePartitionManager::get() {
                let mut options = FindPositionOptions::default();
                options.min_radius = 0.0;
                options.max_radius = 20.0;
                let search_origin = creation_point;
                let _ = partition_manager.find_position_around_with_options(
                    &search_origin,
                    &options,
                    &mut creation_point,
                );
            }

            // Set the object's position and orientation
            {
                let Ok(mut obj_guard) = new_obj.write() else {
                    continue;
                };
                let _ = obj_guard.set_orientation(orientation);
                let _ = obj_guard.set_position(&creation_point);
            }
        }

        // C++ parity: use MiscAudio::m_crateFreeUnit and bind to picker object ID.
        if let Some(audio) = TheAudio::get() {
            let mut audio_event = TheAudio::get_misc_audio().crate_free_unit.clone();
            audio_event.set_object_id(other.get_id());
            audio.add_audio_event(&audio_event);
        }

        Ok(true)
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
    data: &mut UnitCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.required_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbidden_kind_of(
    _ini: &mut INI,
    data: &mut UnitCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.forbidden_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbid_owner_player(
    _ini: &mut INI,
    data: &mut UnitCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_forbid_owner_player = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_building_pickup(
    _ini: &mut INI,
    data: &mut UnitCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_building_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_human_only(
    _ini: &mut INI,
    data: &mut UnitCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_human_only_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_pickup_science(
    _ini: &mut INI,
    data: &mut UnitCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let science_name = first_token(tokens)?;
    let science = get_science_store()
        .map(|store| store.get_science_from_internal_name(science_name))
        .unwrap_or(SCIENCE_INVALID);
    if science == SCIENCE_INVALID {
        return Err(INIError::InvalidData);
    }
    data.base.pickup_science = science as crate::common::science::ScienceType;
    Ok(())
}

fn parse_execute_fx(
    _ini: &mut INI,
    data: &mut UnitCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_fx = Some(first_token(tokens)?.to_string());
    Ok(())
}

fn parse_execute_animation(
    _ini: &mut INI,
    data: &mut UnitCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execution_animation_template = first_token(tokens)?.to_string();
    Ok(())
}

fn parse_execute_animation_time(
    _ini: &mut INI,
    data: &mut UnitCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_display_time_seconds = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_z_rise(
    _ini: &mut INI,
    data: &mut UnitCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_z_rise_per_second = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_fades(
    _ini: &mut INI,
    data: &mut UnitCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_fades = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_unit_count(
    _ini: &mut INI,
    data: &mut UnitCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.unit_count = INI::parse_unsigned_int(first_token(tokens)?)?;
    Ok(())
}

fn parse_unit_name(
    _ini: &mut INI,
    data: &mut UnitCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.unit_type = INI::parse_ascii_string(first_token(tokens)?)?;
    Ok(())
}

const UNIT_CRATE_COLLIDE_FIELDS: &[FieldParse<UnitCrateCollideModuleData>] = &[
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
    FieldParse {
        token: "UnitCount",
        parse: parse_unit_count,
    },
    FieldParse {
        token: "UnitName",
        parse: parse_unit_name,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::KindOf;
    use crate::player::{Player, PlayerIndex};
    use game_engine::common::rts::science::{
        get_science_store, get_science_store_mut, init_science_store, ScienceInfo,
    };
    use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
    use std::sync::{Arc, RwLock};

    struct AudioEventsGuard(bool);

    impl AudioEventsGuard {
        fn disabled() -> Self {
            Self(crate::helpers::set_audio_events_enabled_for_tests(false))
        }
    }

    impl Drop for AudioEventsGuard {
        fn drop(&mut self) {
            crate::helpers::set_audio_events_enabled_for_tests(self.0);
        }
    }

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

    fn create_object_for_player(
        template_name: &str,
        player_index: PlayerIndex,
        position: Coord3D,
    ) -> Arc<RwLock<crate::object::Object>> {
        ensure_template_exists(template_name);
        let team_arc = setup_player_with_team(player_index, "PlayerTeam");
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

        if let Ok(mut manager) = crate::object_manager::get_object_manager().write() {
            let object_id = obj.read().map(|o| o.get_id()).unwrap_or(0);
            let object_position = crate::common::Coord3D::new(position.x, position.y, position.z);
            manager.update_object_position(object_id, object_position);
        }

        obj
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

    #[test]
    fn test_unit_crate_creation() {
        let _lock = crate::test_sync::lock();

        let default_data = UnitCrateCollideModuleData::default();
        assert_eq!(default_data.unit_count, 0);
        assert!(default_data.unit_type.is_empty());

        let module_data = UnitCrateCollideModuleData {
            unit_count: 3,
            unit_type: "Infantry".to_string(),
            ..Default::default()
        };

        let unit_crate = UnitCrateCollide::new(1, module_data);

        assert_eq!(unit_crate.get_version(), 1);
        assert_eq!(
            unit_crate.get_unit_crate_collide_module_data().unit_count,
            3
        );
        assert_eq!(
            unit_crate.get_unit_crate_collide_module_data().unit_type,
            "Infantry"
        );
    }

    #[test]
    fn unit_crate_parser_preserves_cpp_fields() {
        let _lock = crate::test_sync::lock();

        let mut data = UnitCrateCollideModuleData::default();
        parse_unit_count(&mut INI::new(), &mut data, &["3"]).expect("unit count parses");
        parse_unit_name(&mut INI::new(), &mut data, &["AmericaTank"]).expect("unit name parses");
        parse_required_kind_of(&mut INI::new(), &mut data, &["VEHICLE|INFANTRY"])
            .expect("required kindof parses");
        parse_execute_animation_time(&mut INI::new(), &mut data, &["2.5"])
            .expect("animation time parses");

        assert_eq!(data.unit_count, 3);
        assert_eq!(data.unit_type, "AmericaTank");
        assert_ne!(
            data.base.required_kind_of & (1u64 << (KindOf::Vehicle as u32)),
            0
        );
        assert_ne!(
            data.base.required_kind_of & (1u64 << (KindOf::Infantry as u32)),
            0
        );
        assert_eq!(data.base.execute_animation_display_time_seconds, 2.5);
    }

    #[test]
    fn unit_crate_pickup_science_resolves_through_science_store_like_cpp() {
        let _lock = crate::test_sync::lock();

        let expected_science = install_test_science("SCIENCE_UNIT_CRATE_TEST");

        let mut data = UnitCrateCollideModuleData::default();
        let mut ini = INI::new();
        ini.with_inline_source(
            "PickupScience = SCIENCE_UNIT_CRATE_TEST\n\
             UnitCount = 2\n\
             UnitName = Infantry\n\
             RequiredKindOf = VEHICLE|INFANTRY\n\
             End\n",
            |ini| data.parse_from_ini(ini),
        )
        .expect("unit crate ini parses");

        assert_eq!(data.base.pickup_science, expected_science);
        assert_eq!(data.unit_count, 2);
        assert_eq!(data.unit_type, "Infantry");
        assert_ne!(
            data.base.required_kind_of & (1u64 << (KindOf::Vehicle as u32)),
            0
        );
        assert_ne!(
            data.base.required_kind_of & (1u64 << (KindOf::Infantry as u32)),
            0
        );
    }

    #[test]
    fn unit_crate_rejects_unknown_pickup_science_like_cpp() {
        let _lock = crate::test_sync::lock();

        install_test_science("SCIENCE_KNOWN_UNIT_CRATE_TEST");

        let mut data = UnitCrateCollideModuleData::default();
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

    #[test]
    fn unit_crate_creation_preserves_unregistered_default_team() {
        let _lock = crate::test_sync::lock();

        ensure_template_exists("Infantry");
        let team_arc = setup_player_with_team(2, "UnregisteredDefaultTeam");
        let team_id = team_arc.read().expect("team read").get_id();

        let thing_factory = TheThingFactory::get().expect("ThingFactory unavailable");
        let template = TheThingFactory::find_template("Infantry").expect("Template missing");
        let created = thing_factory
            .new_object_with_team_handle(template, Arc::clone(&team_arc))
            .expect("object creates with team handle");

        let created_guard = created.read().expect("created object read");
        assert_eq!(created_guard.get_team_id(), Some(team_id));
        assert_eq!(created_guard.get_controlling_player_id(), Some(2));
    }

    #[test]
    fn test_unit_crate_execute_behavior() {
        let _lock = crate::test_sync::lock();
        let _audio_guard = AudioEventsGuard::disabled();

        ensure_template_exists("Infantry");
        setup_player_with_team(0, "Player1Team");

        let module_data = UnitCrateCollideModuleData {
            unit_count: 2,
            unit_type: "Infantry".to_string(),
            ..Default::default()
        };

        let unit_crate = UnitCrateCollide::new(1, module_data);
        let game_obj = create_object_for_player("Infantry", 0, Coord3D::new(10.0, 20.0, 0.0));

        // Test that the behavior executes successfully
        let result = unit_crate.execute_crate_behavior_internal(&game_obj);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn test_unit_crate_invalid_template() {
        let _lock = crate::test_sync::lock();

        ensure_template_exists("Infantry");
        setup_player_with_team(0, "Player1Team");

        let module_data = UnitCrateCollideModuleData {
            unit_count: 1,
            unit_type: "NonexistentUnit".to_string(),
            ..Default::default()
        };

        let unit_crate = UnitCrateCollide::new(1, module_data);
        let game_obj = create_object_for_player("Infantry", 0, Coord3D::new(10.0, 20.0, 0.0));

        let result = unit_crate.execute_crate_behavior_internal(&game_obj);
        assert!(matches!(result, Ok(false)));
    }

    #[test]
    fn test_collision_handling() {
        let _lock = crate::test_sync::lock();

        ensure_template_exists("Infantry");
        setup_player_with_team(1, "Player1Team");

        let module_data = UnitCrateCollideModuleData::default();
        let mut unit_crate = UnitCrateCollide::new(1, module_data);

        let game_obj = create_object_for_player("Infantry", 1, Coord3D::new(5.0, 5.0, 0.0));

        let collision_pos = Coord3D::new(5.0, 5.0, 0.0);
        let collision_normal = Coord3D::new(0.0, 0.0, 1.0);

        let result = unit_crate.on_collide(Some(&game_obj), &collision_pos, &collision_normal);
        assert!(result.is_ok());
    }
}

impl game_engine::common::system::Snapshotable for UnitCrateCollide {
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
