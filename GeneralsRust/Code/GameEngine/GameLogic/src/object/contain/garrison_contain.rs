//! Garrison Contain Module
//!
//! Contain module for structures that can be garrisoned. Provides advanced
//! containment functionality including healing, garrison points, and combat positioning.

use std::collections::HashMap;
use std::sync::{Arc, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface, OpenContain};
use crate::common::{
    CommandSourceType, Coord3D, DisabledType, GameResult, KindOf, ModelConditionFlags,
    ModelConditionState, ObjectID, ObjectStatusMaskType, ObjectStatusTypes, PlayerMaskType,
    Relationship, UnsignedInt, WeaponBonusConditionType, INVALID_ID,
};
use crate::damage::{BodyDamageType, DamageInfo, DamageType, DeathType};
use crate::error::GameLogicError as GameError;
use crate::helpers::{
    get_game_logic_random_value_real, FindPositionOptions, TheGameClient, TheGameLogic,
    TheGlobalData, TheInGameUI, ThePartitionManager, TheTerrainLogic, TheThingFactory,
    FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS,
};
use crate::modules::{
    AIUpdateInterfaceExt, BodyModuleGuardExt, ContainModuleInterface, ContainWant, ExitDoorType,
    UpdateSleepTime,
};
use crate::object::drawable::{Drawable, DrawableArcExt};
use crate::object::{Object, ObjectId};
use crate::player::{Player, ThePlayerList};
use crate::team::Team;
use crate::weapon::{DamageType as WeaponDamageType, Weapon};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};

/// Maximum number of garrison points
const MAX_GARRISON_POINTS: usize = 40;

/// Maximum number of garrison point conditions (pristine, damaged, really damaged)
const MAX_GARRISON_POINT_CONDITIONS: usize = 3;
const GARRISON_POINT_PRISTINE: usize = 0;
const GARRISON_POINT_DAMAGED: usize = 1;
const GARRISON_POINT_REALLY_DAMAGED: usize = 2;

/// Muzzle flash lifetime in logic frames
const MUZZLE_FLASH_LIFETIME: u32 = 30 / 7; // LOGICFRAMES_PER_SECOND / 7

/// Initial roster configuration for garrison
#[derive(Debug, Clone)]
pub struct InitialRoster {
    pub template_name: String,
    pub count: i32,
}

impl Default for InitialRoster {
    fn default() -> Self {
        Self {
            template_name: String::new(),
            count: 0,
        }
    }
}

/// Configuration data for GarrisonContain module
#[derive(Debug, Clone)]
pub struct GarrisonContainModuleData {
    /// Configuration from parent OpenContain
    pub base: super::OpenContainModuleData,
    /// Whether to heal contained objects
    pub do_heal_objects: bool,
    /// Number of frames for full heal
    pub frames_for_full_heal: f32,
    /// Whether this is a mobile garrison
    pub mobile_garrison: bool,
    /// Whether immune to clear building attacks (toxins/fire)
    pub immune_to_clear_building_attacks: bool,
    /// Whether this is an enclosing container
    pub is_enclosing_container: bool,
    /// Initial roster of units
    pub initial_roster: InitialRoster,
}

impl Default for GarrisonContainModuleData {
    fn default() -> Self {
        let mut base = super::OpenContainModuleData::default();
        base.allow_inside_kind_of = 1u64 << (KindOf::Infantry as u32);

        Self {
            base,
            do_heal_objects: false,
            frames_for_full_heal: 1.0,
            mobile_garrison: false,
            immune_to_clear_building_attacks: false,
            is_enclosing_container: true, // Sensible default for garrison containers
            initial_roster: Default::default(),
        }
    }
}

impl GarrisonContainModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields_allow_unknown(self, GARRISON_CONTAIN_FIELDS)
    }

    pub fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        self.base.parse_from_config(config)?;
        super::parse_with_fields_allow_unknown(config, self, GARRISON_CONTAIN_FIELDS)
    }
}

impl ContainerIniParse for GarrisonContainModuleData {
    fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        GarrisonContainModuleData::parse_from_config(self, config)
    }
}

fn parse_mobile_garrison(
    _ini: &mut INI,
    data: &mut GarrisonContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.mobile_garrison = INI::parse_bool(token)?;
    Ok(())
}

fn parse_heal_objects(
    _ini: &mut INI,
    data: &mut GarrisonContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.do_heal_objects = INI::parse_bool(token)?;
    Ok(())
}

fn parse_time_for_full_heal(
    _ini: &mut INI,
    data: &mut GarrisonContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.frames_for_full_heal = super::parse_duration_frames_real(token)?;
    Ok(())
}

fn parse_initial_roster(
    _ini: &mut INI,
    data: &mut GarrisonContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let name = tokens.first().ok_or(INIError::InvalidData)?;
    let count = match tokens.get(1) {
        Some(token) => INI::parse_int(token)?,
        None => 1,
    };
    data.initial_roster.template_name = name.to_string();
    data.initial_roster.count = count;
    Ok(())
}

fn parse_immune_to_clear_building_attacks(
    _ini: &mut INI,
    data: &mut GarrisonContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.immune_to_clear_building_attacks = INI::parse_bool(token)?;
    Ok(())
}

fn parse_is_enclosing_container(
    _ini: &mut INI,
    data: &mut GarrisonContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.is_enclosing_container = INI::parse_bool(token)?;
    Ok(())
}

const GARRISON_CONTAIN_FIELDS: &[FieldParse<GarrisonContainModuleData>] = &[
    FieldParse {
        token: "MobileGarrison",
        parse: parse_mobile_garrison,
    },
    FieldParse {
        token: "HealObjects",
        parse: parse_heal_objects,
    },
    FieldParse {
        token: "TimeForFullHeal",
        parse: parse_time_for_full_heal,
    },
    FieldParse {
        token: "InitialRoster",
        parse: parse_initial_roster,
    },
    FieldParse {
        token: "ImmuneToClearBuildingAttacks",
        parse: parse_immune_to_clear_building_attacks,
    },
    FieldParse {
        token: "IsEnclosingContainer",
        parse: parse_is_enclosing_container,
    },
];

/// Garrison point condition types
#[derive(Debug, Clone, Copy)]
pub enum GarrisonPointCondition {
    Pristine = 0,
    Damaged = 1,
    ReallyDamaged = 2,
}

/// Garrison point data for tracking occupants
#[derive(Debug)]
pub struct GarrisonPointData {
    /// Occupant object id (INVALID_ID if empty).
    pub object_id: ObjectID,
    /// Current target id (INVALID_ID if none).
    pub target_id: ObjectID,
    /// Frame when placed at this garrison point
    pub place_frame: u32,
    /// Last frame effects were fired
    pub last_effect_frame: u32,
    /// Effect drawable for gun barrels and muzzle flash
    pub effect: Option<Arc<RwLock<Drawable>>>,
    /// Drawable ID for save/load post-process
    pub effect_id: Option<u32>,
}

impl Default for GarrisonPointData {
    fn default() -> Self {
        Self {
            object_id: INVALID_ID,
            target_id: INVALID_ID,
            place_frame: 0,
            last_effect_frame: 0,
            effect: None,
            effect_id: None,
        }
    }
}

/// Station point data for pre-assigned positions
#[derive(Debug, Clone)]
pub struct StationPointData {
    pub occupant_id: Option<ObjectId>,
    pub position: Coord3D,
}

/// Evacuation disposition types
#[derive(Debug, Clone, Copy)]
pub enum EvacDisposition {
    Invalid,
    ToLeft,
    ToRight,
    BurstFromCenter,
}

/// Garrison contain module - handles garrisoned unit containment
#[derive(Debug)]
pub struct GarrisonContain {
    /// Base functionality from OpenContain
    pub base: OpenContain,
    /// Original team before garrison
    original_team: Option<Weak<RwLock<Team>>>,
    /// Garrison point data array
    garrison_point_data: [GarrisonPointData; MAX_GARRISON_POINTS],
    /// Number of garrison points currently in use
    garrison_points_in_use: usize,
    /// Garrison point positions for different damage states
    garrison_points: [[Coord3D; MAX_GARRISON_POINTS]; MAX_GARRISON_POINT_CONDITIONS],
    /// Exit rally point
    exit_rally_point: Coord3D,
    /// Station point list for pre-assigned positions
    station_point_list: Vec<StationPointData>,
    /// Whether station garrison points are initialized
    station_garrison_points_initialized: bool,
    /// Whether garrison points are initialized
    garrison_points_initialized: bool,
    /// Whether to hide garrisoned state from non-allies
    hide_garrisoned_state_from_non_allies: bool,
    /// Whether rally point is valid
    rally_valid: bool,
    /// Evacuation disposition
    evac_disposition: EvacDisposition,
    /// Reference to the owning object
    object_id: ObjectID,
}

impl GarrisonContain {
    fn garrison_point_object(&self, point_index: usize) -> Option<Arc<RwLock<Object>>> {
        let id = self.garrison_point_data.get(point_index)?.object_id;
        if id == INVALID_ID {
            return None;
        }
        TheGameLogic::find_object_by_id(id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
    }

    /// Create a new GarrisonContain module
    pub fn new(
        object: Weak<RwLock<Object>>,
        module_data: &GarrisonContainModuleData,
    ) -> GameResult<Self> {
        let base = OpenContain::new(object.clone(), &module_data.base)?;

        Ok(Self {
            base,
            original_team: None,
            garrison_point_data: std::array::from_fn(|_| GarrisonPointData::default()),
            garrison_points_in_use: 0,
            garrison_points: [[Coord3D::default(); MAX_GARRISON_POINTS];
                MAX_GARRISON_POINT_CONDITIONS],
            exit_rally_point: Coord3D::default(),
            station_point_list: Vec::new(),
            station_garrison_points_initialized: false,
            garrison_points_initialized: false,
            hide_garrisoned_state_from_non_allies: false,
            rally_valid: false,
            evac_disposition: EvacDisposition::BurstFromCenter,
            object_id: object
                .upgrade()
                .and_then(|arc| arc.read().ok().map(|g| g.get_id()))
                .unwrap_or(crate::common::INVALID_ID),
        })
    }

    /// Get the object this module belongs to
    pub fn get_object(&self) -> Option<Arc<RwLock<Object>>> {
        (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        })
    }

    /// Update method called once per frame
    /// C++ reference: GarrisonContain::update() lines 180-220
    pub fn update(&mut self) -> GameResult<UpdateSleepTime> {
        self.base.update()?;

        let contained_objects = self.base.get_contained_items_list()?;
        for obj in contained_objects {
            let is_dead = obj
                .read()
                .map(|guard| guard.is_effectively_dead())
                .unwrap_or(false);
            if is_dead {
                self.remove_from_contain(
                    obj.read()
                        .ok()
                        .map(|g| g.get_id())
                        .unwrap_or(crate::common::INVALID_ID),
                    true,
                )?;
                if let Ok(mut contained) = obj.write() {
                    contained.set_safe_occlusion_frame(
                        TheGameLogic::get_frame() + crate::common::LOGICFRAMES_PER_SECOND * 1000,
                    );
                }
            }
        }

        // Match objects to garrison points (includes effects/targets)
        self.match_objects_to_garrison_points()?;

        // C++ line 185-195: Heal objects if configured to do so
        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner) = owner_obj.read() {
                if let Ok(module_data) = owner.get_garrison_contain_module_data() {
                    if module_data.do_heal_objects {
                        drop(owner);
                        self.heal_objects(&module_data)?;
                    }
                }
            }
        }

        // Move objects with this container if mobile garrison
        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner) = owner_obj.read() {
                if let Ok(module_data) = owner.get_garrison_contain_module_data() {
                    if module_data.mobile_garrison && owner.is_mobile() {
                        drop(owner);
                        self.move_objects_with_me()?;
                    }
                }
            }
        }

        Ok(UpdateSleepTime::None)
    }

    /// Check if this container is valid for the given object
    pub fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        // Garrison has extra checks beyond OpenContain
        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner) = owner_obj.read() {
                if let Some(body) = owner.get_body_module() {
                    if let Ok(body_mod) = body.lock() {
                        if body_mod.get_health() <= 0.0 {
                            return false;
                        }
                        if body_mod.get_damage_state() == BodyDamageType::ReallyDamaged {
                            if !owner.is_kind_of(KindOf::GarrisonableUntilDestroyed) {
                                return false;
                            }
                        }
                    }
                }
            }
        }

        if obj.is_kind_of(KindOf::NoGarrison) {
            return false;
        }

        // Call parent validation
        self.base.is_valid_container_for(obj, check_capacity)
    }

    /// Check if this is a garrisonable unit
    pub fn is_garrisonable(&self) -> bool {
        true
    }

    /// Check if this container can be busted by a bunker buster
    pub fn is_bustable(&self) -> bool {
        true
    }

    /// Check if immune to clear building attacks (toxins, fire, etc.)
    /// Matches C++ GarrisonContain::isImmuneToClearBuildingAttacks
    pub fn is_immune_to_clear_building_attacks(&self) -> bool {
        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner) = owner_obj.read() {
                if let Ok(module_data) = owner.get_garrison_contain_module_data() {
                    return module_data.immune_to_clear_building_attacks;
                }
            }
        }
        false
    }

    /// Check if this is a heal container (not a transport)
    pub fn is_heal_contain(&self) -> bool {
        false
    }

    /// Check if this is a tunnel container
    pub fn is_tunnel_contain(&self) -> bool {
        false
    }

    /// Check if passenger is allowed to fire
    pub fn is_passenger_allowed_to_fire(&self, id: Option<ObjectId>) -> bool {
        let _ = id;
        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner) = owner_obj.read() {
                if owner.is_disabled_by_type(DisabledType::DisabledSubdued) {
                    return false;
                }
            }
        }
        true
    }

    /// Check if this is an enclosing container for the given object
    pub fn is_enclosing_container_for(&self, obj: &Object) -> bool {
        self.is_enclosing_container_for_internal(Some(obj))
    }

    fn is_enclosing_container_for_any(&self) -> bool {
        self.is_enclosing_container_for_internal(None)
    }

    fn is_enclosing_container_for_internal(&self, _obj: Option<&Object>) -> bool {
        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner) = owner_obj.read() {
                if let Ok(module_data) = owner.get_garrison_contain_module_data() {
                    return module_data.is_enclosing_container;
                }
            }
        }
        true
    }

    /// Check if this is a special overlord style container
    pub fn is_special_overlord_style_container(&self) -> bool {
        false
    }

    /// Add object to containment using garrison virtual callbacks.
    pub fn add_to_contain(&mut self, obj_id: ObjectID) -> GameResult<()> {
        let obj = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
            .ok_or("Contain object not found")?;
        let owner = self.get_object();
        if super::should_cancel_containment_after_booby_trap(
            owner.and_then(|o| o.read().ok().map(|g| g.get_id())),
            obj_id,
        ) {
            return Ok(());
        }

        let was_selected = obj
            .read()
            .ok()
            .and_then(|guard| guard.get_drawable())
            .and_then(|drawable| drawable.read().ok().map(|draw| draw.is_selected()))
            .unwrap_or(false);

        let obj_guard = obj.read().map_err(|_| GameError::LockError)?;
        if !self.is_valid_container_for(&obj_guard, true) {
            return Err("Object not valid for this container".into());
        }
        if obj_guard.get_contained_by().is_some() {
            return Ok(());
        }
        drop(obj_guard);

        self.base.add_to_contain_list(obj_id)?;

        if obj
            .read()
            .map(|guard| self.is_enclosing_container_for_internal(Some(&guard)))
            .unwrap_or(false)
        {
            let _ = self.base.add_or_remove_obj_from_world(obj_id, false);
        }

        self.base.redeploy_occupants()?;
        self.on_containing(obj_id, was_selected)?;
        Ok(())
    }

    /// Remove object from containment using garrison virtual callbacks.
    pub fn remove_from_contain(
        &mut self,
        obj_id: ObjectID,
        expose_stealth_units: bool,
    ) -> GameResult<()> {
        let obj = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
            .ok_or("Contain object not found")?;
        if let Some(owner) = self.get_object() {
            let owner_id = owner.read().ok().map(|guard| guard.get_id());
            if let (Some(owner_id), Ok(obj_guard)) = (owner_id, obj.read()) {
                if obj_guard.get_contained_by() != Some(owner_id) {
                    return Ok(());
                }
            }
        }

        let object_id = obj.read().map_err(|_| GameError::LockError)?.get_id();
        self.base.remove_from_contain_list(object_id);

        if expose_stealth_units {
            if let Ok(obj_guard) = obj.read() {
                if let Some(stealth) = obj_guard.get_stealth() {
                    if let Ok(mut stealth_guard) = stealth.lock() {
                        stealth_guard.mark_as_detected();
                    }
                }
            }
        }

        self.base.do_unload_sound();
        self.on_removing(obj_id)?;

        if obj
            .read()
            .map(|guard| self.is_enclosing_container_for_internal(Some(&guard)))
            .unwrap_or(false)
        {
            let _ = self.base.add_or_remove_obj_from_world(obj_id, true);
            if let Some(owner) = self.get_object() {
                if let (Ok(owner_guard), Ok(mut obj_guard)) = (owner.read(), obj.write()) {
                    if let Err(err) = obj_guard.set_position(owner_guard.get_position()) {
                        log::warn!(
                            "GarrisonContain::remove_from_contain failed to place object {}: {}",
                            obj_guard.get_id(),
                            err
                        );
                    }
                }
            }
        }

        if let Some(owner) = self.get_object() {
            if let Ok(owner_guard) = owner.read() {
                if let Ok(mut obj_guard) = obj.write() {
                    obj_guard.set_layer(owner_guard.get_layer());
                }
            }
        }

        Ok(())
    }

    /// Remove all contained objects
    pub fn remove_all_contained(&mut self, expose_stealth_units: bool) -> GameResult<()> {
        if self.base.get_contain_count() > 0 {
            self.validate_rally_point()?;
        }
        let objects = self.base.get_contained_items_list()?;
        for obj in objects {
            self.remove_from_contain(
                obj.read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID),
                expose_stealth_units,
            )?;
        }
        self.recalc_apparent_controlling_player()?;
        Ok(())
    }

    /// Exit object via door
    pub fn exit_object_via_door(
        &mut self,
        exit_id: ObjectID,
        exit_door: ExitDoorType,
    ) -> GameResult<()> {
        let _ = exit_door;
        let exit_obj = TheGameLogic::find_object_by_id(exit_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(exit_id))
            .ok_or("exit object not found")?;
        self.remove_from_contain(exit_id, true)?;

        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner) = owner_obj.read() {
                let mut start_pos = *owner.get_position();
                let mut end_pos = start_pos;
                let exit_angle = owner.get_orientation();

                if matches!(
                    self.evac_disposition,
                    EvacDisposition::ToLeft | EvacDisposition::ToRight
                ) {
                    let scalar = if matches!(self.evac_disposition, EvacDisposition::ToLeft) {
                        1.0
                    } else {
                        -1.0
                    };

                    let geom = owner.get_geometry_info();
                    let half_length = geom.get_major_radius();
                    let half_width = geom.get_minor_radius();

                    let door_offset = Coord3D::new(
                        get_game_logic_random_value_real(-half_length / 4.0, half_length / 4.0),
                        get_game_logic_random_value_real(half_width / 2.0, half_width * 2.0)
                            * scalar,
                        0.0,
                    );
                    let walk_offset = Coord3D::new(
                        get_game_logic_random_value_real(-half_length, half_length),
                        half_width * 10.0 * scalar,
                        0.0,
                    );

                    let cos = exit_angle.cos();
                    let sin = exit_angle.sin();
                    start_pos.x += door_offset.x * cos - door_offset.y * sin;
                    start_pos.y += door_offset.x * sin + door_offset.y * cos;
                    end_pos.x += walk_offset.x * cos - walk_offset.y * sin;
                    end_pos.y += walk_offset.x * sin + walk_offset.y * cos;
                } else {
                    if self.is_enclosing_container_for_any() {
                        if let Ok(mut exit_guard) = exit_obj.write() {
                            if let Err(err) = exit_guard.set_position(&start_pos) {
                                log::debug!(
                                    "GarrisonContain::exit_object set_position failed: {err}"
                                );
                            }
                        }
                    }
                }

                if let Some(terrain) = TheTerrainLogic::get() {
                    start_pos.z = terrain.get_ground_height(start_pos.x, start_pos.y, None);
                    end_pos.z = terrain.get_ground_height(end_pos.x, end_pos.y, None);
                }

                if let Ok(mut exit_guard) = exit_obj.write() {
                    if let Err(err) = exit_guard.set_position(&start_pos) {
                        log::debug!("GarrisonContain::exit_object set_position failed: {err}");
                    }
                    let _ = exit_guard.set_orientation(exit_angle);
                }

                if let Ok(exit_guard) = exit_obj.read() {
                    if let Some(ai) = exit_guard.get_ai_update_interface() {
                        ai.ai_follow_path(
                            &[end_pos],
                            Some(owner.get_id()),
                            CommandSourceType::FromAi,
                        );
                    }
                }
            }
        }

        self.recalc_apparent_controlling_player()?;
        Ok(())
    }

    /// Exit object by budding (no-op for garrison)
    pub fn exit_object_by_budding(
        &mut self,
        _new_obj_id: ObjectID,
        _bud_host_id: ObjectID,
    ) -> GameResult<()> {
        // No-op for garrison contain
        Ok(())
    }

    /// Called when this object starts containing another object
    pub fn on_containing(&mut self, obj_id: ObjectID, was_selected: bool) -> GameResult<()> {
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.base.on_containing(obj_id, was_selected)?;

        // Set object as held and disable
        if let Ok(mut contained) = obj.write() {
            contained.set_disabled_held(true)?;
            contained.set_weapon_bonus_condition(WeaponBonusConditionType::Garrisoned);
        }

        if let Some(owner_obj) = self.get_object() {
            if let Ok(mut owner) = owner_obj.write() {
                owner.set_status(ObjectStatusMaskType::CAN_ATTACK, true);
                if let Ok(module_data) = owner.get_garrison_contain_module_data() {
                    if module_data.is_enclosing_container {
                        if let Ok(mut contained) = obj.write() {
                            if let Err(err) = contained.set_position(owner.get_position()) {
                                log::debug!(
                                    "GarrisonContain::on_containing set_position failed: {err}"
                                );
                            }
                        }
                    }
                }
            }
        }

        // Recalculate apparent controlling player
        self.recalc_apparent_controlling_player()?;

        // If selected, deselect from UI
        if let Ok(contained) = obj.read() {
            if let Some(draw) = contained.get_drawable() {
                let selected = draw
                    .read()
                    .map(|guard| guard.is_selected())
                    .unwrap_or(false);
                if selected {
                    TheInGameUI::deselect_drawable(&draw);
                }
            }
        }

        // Ensure garrison/station points are initialized when first occupied.
        if self.base.get_contain_count() > 0 {
            if self.is_enclosing_container_for_any() {
                let _ = self.load_garrison_points();
            } else {
                let _ = self.load_station_garrison_points();
            }
        }

        Ok(())
    }

    /// Called when removing an object from containment
    pub fn on_removing(&mut self, obj_id: ObjectID) -> GameResult<()> {
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.base.on_removing(obj_id)?;

        if let Ok(contained) = obj.read() {
            if self.is_enclosing_container_for_internal(Some(&contained)) {
                self.remove_object_from_garrison_point(
                    obj.read()
                        .ok()
                        .map(|g| g.get_id())
                        .unwrap_or(crate::common::INVALID_ID),
                    None,
                )?;
            } else {
                self.remove_object_from_station_point(&contained)?;
                if let Some(terrain) = TheTerrainLogic::get() {
                    let pos = contained.get_position();
                    let ground_z = terrain.get_ground_height(pos.x, pos.y, None);
                    drop(contained);
                    if let Ok(mut contained) = obj.write() {
                        let mut adjusted = *contained.get_position();
                        adjusted.z = ground_z;
                        let _ = contained.set_position(&adjusted);
                    }
                }
            }
        }

        if let Ok(mut contained) = obj.write() {
            contained.clear_weapon_bonus_condition(WeaponBonusConditionType::Garrisoned);
        }

        // Clear disabled state
        if let Ok(mut contained) = obj.write() {
            contained.set_disabled_held(false)?;
        }

        if self.base.get_contain_count() == 0 {
            if let Some(owner_obj) = self.get_object() {
                if let Ok(mut owner) = owner_obj.write() {
                    if owner.get_team().is_some() {
                        owner.set_team(self.original_team.as_ref().and_then(|t| t.upgrade()))?;
                        self.original_team = None;
                    }
                    owner.clear_status(ObjectStatusMaskType::CAN_ATTACK);
                    owner.clear_model_condition_state(ModelConditionFlags::GARRISONED);
                }
            }
            self.hide_garrisoned_state_from_non_allies = false;
        } else if self.base.get_stealth_units_contained() != self.base.get_contain_count() {
            self.hide_garrisoned_state_from_non_allies = false;
        }

        if let Ok(mut guard) = obj.write() {
            let current_frame = TheGameLogic::get_frame();
            let occlusion_delay = guard.get_template().get_occlusion_delay();
            guard.set_safe_occlusion_frame(current_frame + occlusion_delay);
        }

        self.recalc_apparent_controlling_player()?;

        Ok(())
    }

    /// Called when selling this container
    pub fn on_selling(&mut self) -> GameResult<()> {
        // Eject all contained objects
        self.remove_all_contained(false)?;
        Ok(())
    }

    /// Handle body damage state change
    pub fn on_body_damage_state_change(
        &mut self,
        _damage_info: Option<&DamageInfo>,
        _old_state: BodyDamageType,
        new_state: BodyDamageType,
    ) -> GameResult<()> {
        // If crossing ReallyDamaged threshold, eject all passengers unless allowed
        if new_state == BodyDamageType::ReallyDamaged {
            let allow_until_destroyed = if let Some(owner) = self.get_object() {
                if let Ok(owner_guard) = owner.read() {
                    owner_guard.is_kind_of(KindOf::GarrisonableUntilDestroyed)
                } else {
                    false
                }
            } else {
                false
            };
            if !allow_until_destroyed && self.base.get_contain_count() > 0 {
                let _ = self.order_all_passengers_to_exit(CommandSourceType::FromAi, false);
            }
        }
        Ok(())
    }

    /// Get apparent controlling player
    pub fn get_apparent_controlling_player(
        &self,
        observing_player: Option<&Player>,
    ) -> Option<Arc<RwLock<Player>>> {
        let my_player = self.get_object().and_then(|owner| {
            owner
                .read()
                .ok()
                .and_then(|guard| guard.get_controlling_player())
        });

        if self.hide_garrisoned_state_from_non_allies {
            if let (Some(original_team), Some(my_player), Some(observer)) = (
                self.original_team.as_ref(),
                my_player.clone(),
                observing_player,
            ) {
                if let Some(observer_team) = observer.get_default_team() {
                    if let Ok(observer_team_guard) = observer_team.read() {
                        let relation = my_player
                            .read()
                            .ok()
                            .map(|guard| guard.get_relationship_with_team(&observer_team_guard))
                            .unwrap_or(Relationship::Neutral);
                        if !matches!(relation, Relationship::Allies) {
                            if let Ok(original_guard) = original_team.upgrade()?.read() {
                                if let Some(controller_id) =
                                    original_guard.get_controlling_player_id()
                                {
                                    if let Ok(list) = ThePlayerList().read() {
                                        return list.get_player(controller_id as i32).cloned();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        my_player
    }

    /// Recalculate apparent controlling player
    pub fn recalc_apparent_controlling_player(&mut self) -> GameResult<()> {
        // Record original team first time
        if self.original_team.is_none() {
            if let Some(owner_obj) = self.get_object() {
                if let Ok(owner) = owner_obj.read() {
                    self.original_team = owner.get_team().map(|t| Arc::downgrade(&t));
                }
            }
        }

        let Some(owner_obj) = self.get_object() else {
            return Ok(());
        };

        let contain_count = self.base.get_contain_count() as usize;
        let contained_objects = self.base.get_contained_items_list().unwrap_or_default();
        let mut hide_garrison = false;
        let mut rider_team: Option<Arc<RwLock<Team>>> = None;

        if contain_count > 0 {
            if let Some(first) = contained_objects.first() {
                if let Ok(rider) = first.read() {
                    let detected = rider.test_status(ObjectStatusTypes::Detected);
                    let stealth_count = contained_objects
                        .iter()
                        .filter_map(|obj| obj.read().ok())
                        .filter(|guard| {
                            guard.test_status(ObjectStatusTypes::Stealthed)
                                && !guard.test_status(ObjectStatusTypes::Detected)
                        })
                        .count();
                    hide_garrison = !detected && stealth_count == contain_count;

                    rider_team = rider
                        .get_controlling_player()
                        .and_then(|player| player.read().ok().and_then(|p| p.get_default_team()));
                }
            }
        }

        if let Ok(mut owner) = owner_obj.write() {
            if owner.get_team().is_none() {
                self.original_team = None;
            }
            if contain_count > 0 {
                if let Some(team) = rider_team {
                    let _ = owner.set_team(Some(team));
                }
                self.hide_garrisoned_state_from_non_allies = hide_garrison;
            } else {
                let _ = owner.set_team(self.original_team.as_ref().and_then(|t| t.upgrade()));
                self.hide_garrisoned_state_from_non_allies = false;
            }
        }

        if let Some(owner) = self.get_object() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(drawable) = owner_guard.get_drawable() {
                    let mut set_model_garrisoned = false;
                    if contain_count > 0 {
                        if let Some(first) = contained_objects.first() {
                            if let Ok(occupant) = first.read() {
                                let detected = occupant.test_status(ObjectStatusTypes::Detected);
                                let local_player = ThePlayerList()
                                    .read()
                                    .ok()
                                    .and_then(|list| list.get_local_player().cloned());
                                let apparent = local_player
                                    .as_ref()
                                    .and_then(|local| local.read().ok())
                                    .and_then(|local| {
                                        self.get_apparent_controlling_player(Some(&local))
                                    });
                                let controlling = owner_guard.get_controlling_player();
                                if detected
                                    || (apparent.is_some()
                                        && controlling.is_some()
                                        && Arc::ptr_eq(
                                            apparent.as_ref().unwrap(),
                                            controlling.as_ref().unwrap(),
                                        ))
                                {
                                    set_model_garrisoned = true;
                                }
                            }
                        }
                    }

                    if set_model_garrisoned {
                        drawable.set_model_condition_state(ModelConditionFlags::GARRISONED);
                    } else {
                        drawable.clear_model_condition_state(ModelConditionFlags::GARRISONED);
                    }

                    if let Some(local_player) = ThePlayerList()
                        .read()
                        .ok()
                        .and_then(|list| list.get_local_player().cloned())
                    {
                        if let Ok(local_guard) = local_player.read() {
                            if let Some(controller) =
                                self.get_apparent_controlling_player(Some(&local_guard))
                            {
                                if let Ok(controller_guard) = controller.read() {
                                    let time_of_day = TheGlobalData::get()
                                        .map(|global| global.get_time_of_day())
                                        .unwrap_or(crate::common::audio::TimeOfDay::Day);
                                    let color = match time_of_day {
                                        crate::common::audio::TimeOfDay::Night => {
                                            controller_guard.get_player_night_color()
                                        }
                                        _ => controller_guard.get_player_color(),
                                    };
                                    if let Ok(mut draw_guard) = drawable.write() {
                                        draw_guard.set_indicator_color(color);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if contain_count > 0 {
            if self.is_enclosing_container_for_any() {
                let _ = self.load_garrison_points();
            } else {
                let _ = self.load_station_garrison_points();
            }
        }

        Ok(())
    }

    /// Check if displayed on control bar
    pub fn is_displayed_on_control_bar(&self) -> bool {
        true
    }

    /// Handle damage event
    pub fn on_damage(&mut self, _info: &mut DamageInfo) -> GameResult<()> {
        Ok(())
    }

    /// Set evacuation disposition
    pub fn set_evac_disposition(&mut self, disp: EvacDisposition) {
        self.evac_disposition = disp;
    }

    /// Redeploy occupants at garrison points
    #[allow(dead_code)]
    fn redeploy_occupants(&mut self) -> GameResult<()> {
        self.add_valid_objects_to_garrison_points()?;
        Ok(())
    }

    /// Handle object creation
    fn on_object_created(&mut self) -> GameResult<()> {
        self.load_garrison_points()?;
        self.load_station_garrison_points()?;

        let Some(owner_obj) = self.get_object() else {
            return Ok(());
        };
        let owner_guard = owner_obj.read().map_err(|_| GameError::LockError)?;
        let module_data = match owner_guard.get_garrison_contain_module_data() {
            Ok(data) => data,
            Err(_) => return Ok(()),
        };
        let roster = module_data.initial_roster.clone();
        if roster.count <= 0 || roster.template_name.is_empty() {
            return Ok(());
        }

        let owner_name = owner_guard.get_name().to_string();
        let Some(contain) = owner_guard.get_contain() else {
            return Ok(());
        };
        let Some(controller) = owner_guard.get_controlling_player() else {
            return Ok(());
        };
        let team = controller
            .read()
            .ok()
            .and_then(|guard| guard.get_default_team());
        let Some(team) = team else {
            return Ok(());
        };
        let team_guard = team.read().map_err(|_| GameError::LockError)?;
        drop(owner_guard);

        let Some(template) = TheThingFactory::find_template(&roster.template_name) else {
            return Err(format!(
                "GarrisonContain::on_object_created: template '{}' not found",
                roster.template_name
            )
            .into());
        };
        let factory = TheThingFactory::get().map_err(|e| e.to_string())?;

        for _ in 0..roster.count {
            let payload = factory.new_object(template.clone(), &*team_guard)?;
            let payload_id = payload.read().map_err(|_| GameError::LockError)?.get_id();
            let mut contain_guard = contain.lock().map_err(|_| GameError::LockError)?;
            if contain_guard.can_contain(payload_id) {
                contain_guard
                    .contain_object(payload_id)
                    .map_err(|e| e.to_string())?;
            } else {
                return Err(format!(
                    "GarrisonContain::on_object_created: {} is full or not valid for payload {}",
                    owner_name, roster.template_name
                )
                .into());
            }
        }

        Ok(())
    }

    /// Validate and pick exit rally point if possible
    fn validate_rally_point(&mut self) -> GameResult<()> {
        let Some(owner_arc) = self.get_object() else {
            return Ok(());
        };
        let owner_guard = owner_arc.read().map_err(|_| GameError::LockError)?;
        let owner_id = owner_guard.get_id();

        if self.rally_valid {
            let mut result = Coord3D::default();
            let mut options = FindPositionOptions::default();
            options.flags = FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS;
            options.min_radius = 0.0;
            options.max_radius = 0.0;
            options.ignore_object_id = Some(owner_id);
            options.relationship_object_id = Some(owner_id);

            let mut valid = false;
            if let Some(partition) = ThePartitionManager::get() {
                valid = partition.find_position_around_with_options(
                    &self.exit_rally_point,
                    &options,
                    &mut result,
                );
            }
            if !valid {
                self.rally_valid = false;
            }
        }

        if !self.rally_valid {
            let mut options = FindPositionOptions::default();
            options.flags = FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS;
            options.min_radius = owner_guard.get_geometry_info().get_bounding_circle_radius();
            options.max_radius = options.min_radius * 1.8;
            options.ignore_object_id = Some(owner_id);
            options.relationship_object_id = Some(owner_id);

            if let Some(partition) = ThePartitionManager::get() {
                self.rally_valid = partition.find_position_around_with_options(
                    owner_guard.get_position(),
                    &options,
                    &mut self.exit_rally_point,
                );
            } else {
                self.rally_valid = false;
            }
        }
        Ok(())
    }

    /// Calculate best garrison position
    #[allow(dead_code)]
    fn calc_best_garrison_position(&self, source_pos: &mut Coord3D, target_pos: &Coord3D) -> bool {
        if !self.garrison_points_initialized {
            return false;
        }
        let condition_index = self.find_condition_index();
        let point_index = self.find_closest_free_garrison_point_index(condition_index, target_pos);
        if point_index < 0 {
            return false;
        }
        let point_index = point_index as usize;
        if point_index >= MAX_GARRISON_POINTS {
            return false;
        }
        *source_pos = self.garrison_points[condition_index][point_index];
        true
    }

    /// Attempt best fire point position for object with weapon against victim
    fn attempt_best_fire_point_position(
        &mut self,
        source_id: ObjectID,
        weapon: &Weapon,
        victim_id: ObjectID,
    ) -> bool {
        let Some(victim) = crate::helpers::TheGameLogic::find_object_by_id(victim_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(victim_id))
        else {
            return false;
        };
        let Some(source) = crate::helpers::TheGameLogic::find_object_by_id(source_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(source_id))
        else {
            return false;
        };

        if self.load_garrison_points().is_err() {
            return false;
        }
        let target_pos = victim.read().ok().map(|guard| *guard.get_position());
        let Some(target_pos) = target_pos else {
            return false;
        };

        let current_index = source
            .read()
            .ok()
            .and_then(|guard| self.get_object_garrison_point_index(&guard));
        if let Some(idx) = current_index {
            let _ = self.remove_object_from_garrison_point(source_id, Some(idx));
        }

        let _ =
            self.put_object_at_best_garrison_point(source_id, Some(victim_id), Some(&target_pos));

        let in_range = weapon.is_within_attack_range(source_id, Some(victim_id), None);
        if in_range {
            return true;
        }

        if let Some(idx) = source
            .read()
            .ok()
            .and_then(|guard| self.get_object_garrison_point_index(&guard))
        {
            let _ = self.remove_object_from_garrison_point(source_id, Some(idx));
        }
        false
    }

    /// Attempt best fire point position for object with weapon against position
    fn attempt_best_fire_point_position_coord(
        &mut self,
        source_id: ObjectID,
        weapon: &Weapon,
        target_pos: &Coord3D,
    ) -> bool {
        let Some(source) = crate::helpers::TheGameLogic::find_object_by_id(source_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(source_id))
        else {
            return false;
        };

        if self.load_garrison_points().is_err() {
            return false;
        }
        let current_index = source
            .read()
            .ok()
            .and_then(|guard| self.get_object_garrison_point_index(&guard));
        if let Some(idx) = current_index {
            let _ = self.remove_object_from_garrison_point(source_id, Some(idx));
        }

        let _ = self.put_object_at_best_garrison_point(source_id, None, Some(target_pos));
        let in_range = weapon.is_within_attack_range(source_id, None, Some(target_pos));
        if in_range {
            return true;
        }

        if let Some(idx) = source
            .read()
            .ok()
            .and_then(|guard| self.get_object_garrison_point_index(&guard))
        {
            let _ = self.remove_object_from_garrison_point(source_id, Some(idx));
        }
        false
    }

    /// Update effects (muzzle flashes, etc.)
    fn update_effects(&mut self) -> GameResult<()> {
        let current_frame = TheGameLogic::get_frame();
        let contained_objects = self.base.get_contained_items_list()?;

        // Check for objects that fired last frame and create muzzle flash
        for obj in &contained_objects {
            if let Ok(contained) = obj.read() {
                let last_shot_frame = contained.get_last_shot_fired_frame();

                // Did object fire last frame?
                if current_frame > 0 && last_shot_frame == current_frame - 1 {
                    let garrison_index = self.get_object_garrison_point_index(&contained);

                    if let Some(garrison_index) = garrison_index {
                        // Set muzzle flash effect
                        if let Some(ref mut effect) =
                            self.garrison_point_data[garrison_index].effect
                        {
                            // Check if weapon should show muzzle flash
                            if let Some((weapon, _slot)) = contained.get_current_weapon() {
                                let damage_type = weapon.get_damage_type();
                                // No muzzle flash for poison weapons
                                if damage_type != WeaponDamageType::Poison {
                                    if let Ok(mut eff) = effect.write() {
                                        eff.set_model_condition_state(ModelConditionState::FiringA);
                                        self.garrison_point_data[garrison_index]
                                            .last_effect_frame = current_frame;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Remove old firing effects
        for i in 0..MAX_GARRISON_POINTS {
            if let Some(ref mut effect) = self.garrison_point_data[i].effect {
                let last_effect_frame = self.garrison_point_data[i].last_effect_frame;

                // Clear muzzle flash after lifetime expires
                if last_effect_frame != 0
                    && current_frame > last_effect_frame + MUZZLE_FLASH_LIFETIME
                {
                    if let Ok(mut eff) = effect.write() {
                        eff.clear_model_condition_state(ModelConditionState::FiringA);
                        self.garrison_point_data[i].last_effect_frame = 0;
                    }
                }
            }
        }

        Ok(())
    }

    /// Load garrison point positions from art data
    fn load_garrison_points(&mut self) -> GameResult<()> {
        if self.garrison_points_initialized {
            return Ok(());
        }

        if !self.is_enclosing_container_for_any() {
            self.garrison_points_initialized = true;
            return Ok(());
        }

        let Some(owner_obj) = self.get_object() else {
            return Ok(());
        };
        let Ok(mut owner) = owner_obj.write() else {
            return Ok(());
        };

        let base_pos = *owner.get_position();

        for condition_index in 0..MAX_GARRISON_POINT_CONDITIONS {
            for i in 0..MAX_GARRISON_POINTS {
                self.garrison_points[condition_index][i] = base_pos;
            }
        }

        let Some(drawable) = owner.get_drawable() else {
            self.garrison_points_initialized = true;
            return Ok(());
        };

        let original_flags = drawable.get_model_condition_flags();

        let mut clear_flags = ModelConditionFlags::empty();
        clear_flags.set(ModelConditionFlags::REALLY_DAMAGED, true);
        clear_flags.set(ModelConditionFlags::RUBBLE, true);
        clear_flags.set(ModelConditionFlags::SPECIAL_DAMAGED, true);
        clear_flags.set(ModelConditionFlags::DAMAGED, true);

        let mut set_flags = ModelConditionFlags::empty();
        set_flags.set(ModelConditionFlags::GARRISONED, true);

        // pristine garrisoned
        let _ = owner.clear_and_set_model_condition_flags(clear_flags, set_flags);
        let positions = if let Ok(draw_guard) = drawable.read() {
            draw_guard.get_pristine_bone_positions("FIREPOINT", 0, MAX_GARRISON_POINTS)
        } else {
            Vec::new()
        };
        for (i, pos) in positions.iter().enumerate() {
            if i < MAX_GARRISON_POINTS {
                self.garrison_points[GARRISON_POINT_PRISTINE][i] = *pos;
            }
        }

        // damaged garrisoned
        let mut set_damaged = ModelConditionFlags::empty();
        set_damaged.set(ModelConditionFlags::DAMAGED, true);
        let _ = owner.clear_and_set_model_condition_flags(clear_flags, set_damaged);
        let positions = if let Ok(draw_guard) = drawable.read() {
            draw_guard.get_pristine_bone_positions("FIREPOINT", 0, MAX_GARRISON_POINTS)
        } else {
            Vec::new()
        };
        for (i, pos) in positions.iter().enumerate() {
            if i < MAX_GARRISON_POINTS {
                self.garrison_points[GARRISON_POINT_DAMAGED][i] = *pos;
            }
        }

        // really damaged garrisoned
        let mut clear_really = ModelConditionFlags::empty();
        clear_really.set(ModelConditionFlags::RUBBLE, true);
        clear_really.set(ModelConditionFlags::SPECIAL_DAMAGED, true);
        clear_really.set(ModelConditionFlags::DAMAGED, true);
        let mut set_really = ModelConditionFlags::empty();
        set_really.set(ModelConditionFlags::REALLY_DAMAGED, true);
        let _ = owner.clear_and_set_model_condition_flags(clear_really, set_really);
        let positions = if let Ok(draw_guard) = drawable.read() {
            draw_guard.get_pristine_bone_positions("FIREPOINT", 0, MAX_GARRISON_POINTS)
        } else {
            Vec::new()
        };
        for (i, pos) in positions.iter().enumerate() {
            if i < MAX_GARRISON_POINTS {
                self.garrison_points[GARRISON_POINT_REALLY_DAMAGED][i] = *pos;
            }
        }

        let _ =
            owner.clear_and_set_model_condition_flags(ModelConditionFlags::all(), original_flags);

        self.garrison_points_initialized = true;
        Ok(())
    }

    /// Put object at best garrison point for given target
    fn put_object_at_best_garrison_point(
        &mut self,
        obj_id: ObjectID,
        target_id: Option<ObjectID>,
        target_pos: Option<&Coord3D>,
    ) -> GameResult<()> {
        let condition_index = self.find_condition_index();

        if let Some(pos) = target_pos {
            let point_index = self.find_closest_free_garrison_point_index(condition_index, pos);
            if point_index != -1 {
                self.put_object_at_garrison_point(
                    obj_id,
                    target_id,
                    condition_index,
                    point_index as usize,
                )?;
            }
        }

        Ok(())
    }

    /// Put object at specified garrison point
    fn put_object_at_garrison_point(
        &mut self,
        obj_id: ObjectID,
        target_id: Option<ObjectId>,
        condition_index: usize,
        point_index: usize,
    ) -> GameResult<()> {
        let obj = TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
            .ok_or("garrison point object not found")?;
        if point_index >= MAX_GARRISON_POINTS || condition_index >= MAX_GARRISON_POINT_CONDITIONS {
            return Err("Invalid garrison point index".into());
        }

        if self.garrison_point_data[point_index].object_id != INVALID_ID {
            return Err("Garrison point is not empty".into());
        }

        // Set object position
        let pos = self.garrison_points[condition_index][point_index];
        if let Ok(mut object) = obj.write() {
            if let Err(err) = object.set_position(&pos) {
                log::debug!(
                    "GarrisonContain::put_object_at_garrison_point set_position failed: {err}"
                );
            }
        }

        // Save garrison point data (ID-first occupancy).
        let obj_id = obj
            .read()
            .ok()
            .map(|guard| guard.get_id())
            .unwrap_or(INVALID_ID);
        self.garrison_point_data[point_index].object_id = obj_id;
        self.garrison_point_data[point_index].target_id = target_id.unwrap_or(INVALID_ID);
        self.garrison_point_data[point_index].place_frame = TheGameLogic::get_frame();
        self.garrison_points_in_use += 1;

        // Create effect drawable (gun barrel)
        if let Ok(obj_guard) = obj.read() {
            self.create_garrison_effect(point_index, &pos, &*obj_guard)?;
        }

        Ok(())
    }

    /// Remove object from garrison point
    fn remove_object_from_garrison_point(
        &mut self,
        obj_id: ObjectID,
        index: Option<usize>,
    ) -> GameResult<()> {
        let obj = TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id));
        let point_index = if let Some(idx) = index {
            idx
        } else {
            let mut found_index = None;
            for i in 0..MAX_GARRISON_POINTS {
                if self.garrison_point_data[i].object_id == obj_id {
                    found_index = Some(i);
                    break;
                }
            }
            match found_index {
                Some(found_index) => found_index,
                None => return Ok(()),
            }
        };

        if point_index >= MAX_GARRISON_POINTS {
            return Err("Invalid garrison point index".into());
        }

        if let (Some(owner), Some(obj)) = (self.get_object(), obj) {
            if let (Ok(owner_guard), Ok(mut obj_guard)) = (owner.read(), obj.write()) {
                if let Err(err) = obj_guard.set_position(owner_guard.get_position()) {
                    log::debug!(
                        "GarrisonContain::remove_object_from_garrison_point set_position failed: {err}"
                    );
                }
            }
        }

        self.garrison_point_data[point_index].object_id = INVALID_ID;
        self.garrison_point_data[point_index].target_id = INVALID_ID;
        self.garrison_point_data[point_index].place_frame = 0;
        self.garrison_point_data[point_index].last_effect_frame = 0;

        if let Some(effect_id) = self.garrison_point_data[point_index].effect_id {
            if let Some(client) = TheGameClient::get() {
                client.destroy_drawable(effect_id);
            }
        }
        self.garrison_point_data[point_index].effect = None;
        self.garrison_point_data[point_index].effect_id = None;

        if self.garrison_points_in_use > 0 {
            self.garrison_points_in_use -= 1;
        }

        Ok(())
    }

    /// Add valid objects to garrison points
    fn add_valid_objects_to_garrison_points(&mut self) -> GameResult<()> {
        let contained_objects = self.base.get_contained_items_list()?;

        for obj in contained_objects {
            if let Ok(contained) = obj.read() {
                // Check if object is attacking
                if contained.is_attacking() {
                    let obj_id = contained.get_id();
                    // Get target position (victim or target position)
                    if let Some(victim_id) = contained.get_current_victim_id() {
                        if let Some(target_obj) = TheGameLogic::find_object_by_id(victim_id)
                            .or_else(|| {
                                crate::object::registry::OBJECT_REGISTRY.get_object(victim_id)
                            })
                        {
                            if let Ok(target) = target_obj.read() {
                                let target_pos = *target.get_position();
                                drop(target);
                                drop(contained);
                                self.put_object_at_best_garrison_point(
                                    obj_id,
                                    Some(victim_id),
                                    Some(&target_pos),
                                )?;
                            }
                        }
                    } else if let Some(target_pos) = contained.get_current_victim_pos() {
                        drop(contained);
                        self.put_object_at_best_garrison_point(obj_id, None, Some(&target_pos))?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Remove invalid objects from garrison points
    fn remove_invalid_objects_from_garrison_points(&mut self) -> GameResult<()> {
        if self.garrison_points_in_use == 0 {
            return Ok(());
        }

        let mut to_remove = Vec::new();

        for i in 0..MAX_GARRISON_POINTS {
            if let Some(obj) = self.garrison_point_object(i) {
                if let Ok(contained) = obj.read() {
                    let mut target_is_valid = true;

                    // Check if object has a valid target
                    if let Some(goal_id) = contained.get_goal_object_id() {
                        // Check if weapon can still reach target
                        if let Some((weapon, _slot)) = contained.get_current_weapon() {
                            if !weapon.is_within_attack_range(
                                contained.get_id(),
                                Some(goal_id),
                                None,
                            ) {
                                target_is_valid = false;
                            }
                        } else {
                            target_is_valid = false;
                        }
                    }

                    // If not attacking or target invalid, remove from garrison point
                    if !contained.is_attacking() || !target_is_valid {
                        to_remove.push((obj.clone(), i));
                    }
                }
            }
        }

        // Remove invalid objects
        for (obj, index) in to_remove {
            self.remove_object_from_garrison_point(
                obj.read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID),
                Some(index),
            )?;
        }

        Ok(())
    }

    /// Track targets and keep attackers at closest garrison points
    fn track_targets(&mut self) -> GameResult<()> {
        // Only track if this is an enclosing container
        if !self.is_enclosing_container_for_any() {
            return Ok(());
        }

        let condition_index = self.find_condition_index();
        let contained_objects = self.base.get_contained_items_list()?;

        for obj in contained_objects {
            if let Ok(contained) = obj.read() {
                // Only consider objects at garrison points
                let our_index = self.get_object_garrison_point_index(&contained);
                if let Some(our_index) = our_index {
                    // Get current target
                    let victim_pos = if let Some(victim_id) = contained.get_current_victim_id() {
                        TheGameLogic::find_object_by_id(victim_id)
                            .or_else(|| {
                                crate::object::registry::OBJECT_REGISTRY.get_object(victim_id)
                            })
                            .and_then(|v| v.read().ok().map(|g| *g.get_position()))
                    } else {
                        contained.get_current_victim_pos()
                    };

                    if let Some(target_pos) = victim_pos {
                        let our_pos = *contained.get_position();

                        // Find closest free garrison point
                        let new_index = self
                            .find_closest_free_garrison_point_index(condition_index, &target_pos);
                        if new_index != -1 {
                            let new_index = new_index as usize;

                            // Calculate distances
                            let current_dist_sq = self.calc_dist_sqr(&target_pos, &our_pos);
                            let new_dist_sq = self.calc_dist_sqr(
                                &target_pos,
                                &self.garrison_points[condition_index][new_index],
                            );

                            // Switch to closer garrison point
                            if new_dist_sq < current_dist_sq {
                                let obj_clone = obj.clone();
                                drop(contained);
                                let rem_id = obj_clone
                                    .read()
                                    .ok()
                                    .map(|g| g.get_id())
                                    .unwrap_or(crate::common::INVALID_ID);
                                self.remove_object_from_garrison_point(rem_id, Some(our_index))?;

                                let target_id = obj_clone
                                    .read()
                                    .ok()
                                    .and_then(|c| c.get_current_victim_id());

                                let put_id = obj_clone
                                    .read()
                                    .ok()
                                    .map(|g| g.get_id())
                                    .unwrap_or(crate::common::INVALID_ID);
                                self.put_object_at_garrison_point(
                                    put_id,
                                    target_id,
                                    condition_index,
                                    new_index,
                                )?;
                            }

                            // Orient effect drawable towards target
                            if let Some(ref mut effect) = self.garrison_point_data[our_index].effect
                            {
                                // Calculate orientation towards target
                                let dx = target_pos.x - our_pos.x;
                                let dy = target_pos.y - our_pos.y;
                                let angle = dy.atan2(dx);

                                if let Ok(mut eff) = effect.write() {
                                    eff.set_orientation(angle);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Match objects to garrison points every frame
    fn match_objects_to_garrison_points(&mut self) -> GameResult<()> {
        if self.is_enclosing_container_for_any() {
            self.remove_invalid_objects_from_garrison_points()?;
            self.add_valid_objects_to_garrison_points()?;
            self.update_effects()?;
            self.track_targets()?;
        } else {
            self.position_objects_at_station_garrison_points()?;
        }
        Ok(())
    }

    /// Position objects at their assigned station garrison points
    fn position_objects_at_station_garrison_points(&mut self) -> GameResult<()> {
        if !self.station_garrison_points_initialized {
            self.load_station_garrison_points()?;
        }
        let contained_objects = self.base.get_contained_items_list()?;
        for obj in contained_objects {
            let Ok(contained) = obj.read() else {
                continue;
            };
            let mut found = false;
            for station in &self.station_point_list {
                if station.occupant_id == Some(contained.get_id()) {
                    if let Ok(mut guard) = obj.write() {
                        if let Err(err) = guard.set_position(&station.position) {
                            log::debug!(
                                "GarrisonContain::position_objects_at_station_garrison_points set_position failed: {err}"
                            );
                        }
                    }
                    found = true;
                    break;
                }
            }

            if !found {
                if self.pick_a_station_for_me(&contained) {
                    for station in &self.station_point_list {
                        if station.occupant_id == Some(contained.get_id()) {
                            if let Ok(mut guard) = obj.write() {
                                if let Err(err) = guard.set_position(&station.position) {
                                    log::debug!(
                                        "GarrisonContain::position_objects_at_station_garrison_points set_position failed: {err}"
                                    );
                                }
                            }
                            found = true;
                            break;
                        }
                    }
                }
            }

            if !found {
                return Err("No vacant station garrison point".into());
            }
        }
        Ok(())
    }

    /// Load station garrison points from art data
    fn load_station_garrison_points(&mut self) -> GameResult<()> {
        if self.station_garrison_points_initialized {
            return Ok(());
        }

        if self.is_enclosing_container_for_any() {
            self.station_garrison_points_initialized = true;
            return Ok(());
        }

        let Some(owner_obj) = self.get_object() else {
            return Ok(());
        };
        let Ok(mut owner) = owner_obj.write() else {
            return Ok(());
        };
        let Some(drawable) = owner.get_drawable() else {
            self.station_garrison_points_initialized = true;
            return Ok(());
        };

        let original_flags = drawable.get_model_condition_flags();
        let mut clear_flags = ModelConditionFlags::empty();
        clear_flags.set(ModelConditionFlags::REALLY_DAMAGED, true);
        clear_flags.set(ModelConditionFlags::RUBBLE, true);
        clear_flags.set(ModelConditionFlags::SPECIAL_DAMAGED, true);
        clear_flags.set(ModelConditionFlags::DAMAGED, true);

        let mut set_flags = ModelConditionFlags::empty();
        set_flags.set(ModelConditionFlags::GARRISONED, true);
        let _ = owner.clear_and_set_model_condition_flags(clear_flags, set_flags);

        let contain_max = owner
            .get_garrison_contain_module_data()
            .map(|d| d.base.contain_max)
            .unwrap_or(MAX_GARRISON_POINTS as i32);
        let max_points = if contain_max <= 0 {
            MAX_GARRISON_POINTS
        } else {
            contain_max.min(MAX_GARRISON_POINTS as i32) as usize
        };

        let positions = if let Ok(draw_guard) = drawable.read() {
            draw_guard.get_pristine_bone_positions("STATION", 0, max_points)
        } else {
            Vec::new()
        };

        self.station_point_list.clear();
        for pos in positions {
            self.station_point_list.push(StationPointData {
                occupant_id: None,
                position: pos,
            });
        }

        let _ =
            owner.clear_and_set_model_condition_flags(ModelConditionFlags::all(), original_flags);
        self.station_garrison_points_initialized = true;
        Ok(())
    }

    /// Pick a station for the given object
    fn pick_a_station_for_me(&mut self, obj: &Object) -> bool {
        let obj_id = obj.get_id();
        for station in &mut self.station_point_list {
            if station.occupant_id.is_none() {
                station.occupant_id = Some(obj_id);
                return true;
            }
        }
        false
    }

    /// Remove object from station point
    fn remove_object_from_station_point(&mut self, obj: &Object) -> GameResult<()> {
        let obj_id = obj.get_id();
        for station in &mut self.station_point_list {
            if station.occupant_id == Some(obj_id) {
                station.occupant_id = None;
            }
        }
        Ok(())
    }

    /// Find condition index based on current damage state
    fn find_condition_index(&self) -> usize {
        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner) = owner_obj.read() {
                if let Some(body) = owner.get_body_module() {
                    if let Ok(body_mod) = body.lock() {
                        match body_mod.get_damage_state() {
                            BodyDamageType::Pristine => GarrisonPointCondition::Pristine as usize,
                            BodyDamageType::Damaged => GarrisonPointCondition::Damaged as usize,
                            BodyDamageType::ReallyDamaged | BodyDamageType::Rubble => {
                                GarrisonPointCondition::ReallyDamaged as usize
                            }
                        }
                    } else {
                        GarrisonPointCondition::Pristine as usize
                    }
                } else {
                    GarrisonPointCondition::Pristine as usize
                }
            } else {
                GarrisonPointCondition::Pristine as usize
            }
        } else {
            GarrisonPointCondition::Pristine as usize
        }
    }

    /// Get object garrison point index
    fn get_object_garrison_point_index(&self, obj: &Object) -> Option<usize> {
        let obj_id = obj.get_id();
        for i in 0..MAX_GARRISON_POINTS {
            if let Some(point_obj) = self.garrison_point_object(i) {
                if point_obj.read().ok().map(|guard| guard.get_id()) == Some(obj_id) {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Find closest free garrison point to target position
    fn find_closest_free_garrison_point_index(
        &self,
        condition_index: usize,
        target_pos: &Coord3D,
    ) -> i32 {
        if !self.garrison_points_initialized || self.garrison_points_in_use == MAX_GARRISON_POINTS {
            return -1;
        }

        let mut closest_index = -1i32;
        let mut closest_dist_sq = f32::MAX;

        for i in 0..MAX_GARRISON_POINTS {
            if self.garrison_point_data[i].object_id == INVALID_ID {
                let dist_sq =
                    self.calc_dist_sqr(target_pos, &self.garrison_points[condition_index][i]);
                if dist_sq < closest_dist_sq {
                    closest_dist_sq = dist_sq;
                    closest_index = i as i32;
                }
            }
        }

        closest_index
    }

    /// Calculate squared distance between two points
    fn calc_dist_sqr(&self, a: &Coord3D, b: &Coord3D) -> f32 {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let dz = a.z - b.z;
        dx * dx + dy * dy + dz * dz
    }

    /// Get garrison point index for object (public accessor for other modules)
    pub fn get_garrison_point_index_for_object(&self, obj: &Object) -> Option<usize> {
        self.get_object_garrison_point_index(obj)
    }

    /// Check if unit can fire from current garrison point at target
    pub fn can_fire_at_target_from_garrison(&self, obj: &Object, _target_pos: &Coord3D) -> bool {
        self.get_object_garrison_point_index(obj)
            .map(|garrison_index| garrison_index < MAX_GARRISON_POINTS)
            .unwrap_or(false)
    }

    /// Heal all contained objects
    fn heal_objects(&mut self, module_data: &GarrisonContainModuleData) -> GameResult<()> {
        if !module_data.do_heal_objects {
            return Ok(());
        }

        let contained_objects = self.base.get_contained_items_list()?;
        for obj in contained_objects {
            self.heal_single_object(obj, module_data.frames_for_full_heal)?;
        }
        Ok(())
    }

    /// Heal a single contained object
    /// C++ reference: GarrisonContain.cpp healContained() lines 280-310
    fn heal_single_object(
        &mut self,
        obj: Arc<RwLock<Object>>,
        frames_for_full_heal: f32,
    ) -> GameResult<()> {
        if frames_for_full_heal <= 0.0 {
            return Ok(());
        }

        if let Ok(obj_guard) = obj.read() {
            // C++ line 285-290: Get body module and health values
            if let Some(body) = obj_guard.get_body_module() {
                if let Ok(body_guard) = body.lock() {
                    let max_health = body_guard.get_max_health();
                    let current_health = body_guard.get_health();

                    // C++ line 295: Only heal if not at max health
                    if current_health < max_health {
                        let current_frame = TheGameLogic::get_frame();
                        let contained_by_frame = obj_guard.get_contained_by_frame();
                        let frames_contained = current_frame.saturating_sub(contained_by_frame);
                        let frames_for_full = frames_for_full_heal.max(1.0);
                        let heal_amount = if (frames_contained as f32) >= frames_for_full {
                            max_health
                        } else {
                            max_health / frames_for_full
                        };

                        // C++ line 302-305: Create healing damage info
                        let mut heal_info = DamageInfo::new();
                        heal_info.input.damage_type = DamageType::Healing;
                        heal_info.input.death_type = DeathType::None;
                        heal_info.input.amount = heal_amount;
                        heal_info.sync_from_input();

                        // C++ line 307: Apply healing via body module
                        drop(body_guard);
                        drop(obj_guard);
                        if let Ok(mut obj_write) = obj.write() {
                            obj_write.attempt_damage(&mut heal_info)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Move all contained objects with this container (mobile garrison)
    fn move_objects_with_me(&mut self) -> GameResult<()> {
        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner) = owner_obj.read() {
                let pos = owner.get_position();
                for object_id in self.base.get_contained_object_ids().to_vec() {
                    if let Some(obj) = TheGameLogic::find_object_by_id(object_id)
                        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(object_id))
                    {
                        if let Ok(mut contained) = obj.write() {
                            if let Err(err) = contained.set_position(pos) {
                                log::debug!(
                                    "GarrisonContain::move_objects_with_me set_position failed: {err}"
                                );
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Create garrison effect drawable (gun barrel)
    fn create_garrison_effect(
        &mut self,
        point_index: usize,
        pos: &Coord3D,
        obj: &Object,
    ) -> GameResult<()> {
        if !self.is_enclosing_container_for(obj) {
            return Ok(());
        }
        let Some(template) = TheThingFactory::find_template("GarrisonGun") else {
            return Err("GarrisonContain: template 'GarrisonGun' not found".into());
        };
        let Some(client) = TheGameClient::get() else {
            return Ok(());
        };
        let drawable_id = client.create_drawable(template.as_ref());
        client.set_drawable_position(drawable_id, pos);
        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner) = owner_obj.read() {
                client.set_drawable_shroud_status_object_id(drawable_id, owner.get_id());
            }
        }

        self.garrison_point_data[point_index].effect = client.get_drawable_arc(drawable_id);
        self.garrison_point_data[point_index].effect_id = Some(drawable_id);
        self.garrison_point_data[point_index].last_effect_frame = 0;
        Ok(())
    }

    /// Serialize state for save/load
    pub fn save_state(&self) -> GameResult<HashMap<String, Vec<u8>>> {
        let mut state = self.base.save_state()?;

        fn push_f32(out: &mut Vec<u8>, value: f32) {
            out.extend_from_slice(&value.to_le_bytes());
        }

        fn push_u32(out: &mut Vec<u8>, value: u32) {
            out.extend_from_slice(&value.to_le_bytes());
        }

        // Save garrison points in use
        state.insert(
            "garrison_points_in_use".to_string(),
            (self.garrison_points_in_use as u32).to_le_bytes().to_vec(),
        );

        // Save original team + hide flag
        let original_team_id = self
            .original_team
            .as_ref()
            .and_then(|team| team.upgrade())
            .and_then(|team| team.read().ok().map(|guard| guard.get_id()))
            .unwrap_or(crate::team::TEAM_ID_INVALID);
        state.insert(
            "original_team_id".to_string(),
            (original_team_id as u32).to_le_bytes().to_vec(),
        );
        state.insert(
            "hide_garrisoned_state".to_string(),
            vec![if self.hide_garrisoned_state_from_non_allies {
                1
            } else {
                0
            }],
        );

        // Save garrison point data
        let mut point_data = Vec::with_capacity(MAX_GARRISON_POINTS * 5 * 4);
        for point in &self.garrison_point_data {
            let object_id = point.object_id;
            let target_id = point.target_id;
            let effect_id = point.effect_id.unwrap_or(0);
            push_u32(&mut point_data, object_id as u32);
            push_u32(&mut point_data, target_id as u32);
            push_u32(&mut point_data, point.place_frame);
            push_u32(&mut point_data, point.last_effect_frame);
            push_u32(&mut point_data, effect_id);
        }
        state.insert("garrison_point_data".to_string(), point_data);

        // Save garrison point positions
        let mut garrison_points_data =
            Vec::with_capacity(MAX_GARRISON_POINT_CONDITIONS * MAX_GARRISON_POINTS * 3 * 4);
        for condition in &self.garrison_points {
            for point in condition {
                push_f32(&mut garrison_points_data, point.x);
                push_f32(&mut garrison_points_data, point.y);
                push_f32(&mut garrison_points_data, point.z);
            }
        }
        state.insert("garrison_points".to_string(), garrison_points_data);

        state.insert(
            "garrison_points_initialized".to_string(),
            vec![if self.garrison_points_initialized {
                1
            } else {
                0
            }],
        );
        // Save rally info
        state.insert(
            "rally_valid".to_string(),
            vec![if self.rally_valid { 1 } else { 0 }],
        );
        let mut rally_bytes = Vec::with_capacity(12);
        push_f32(&mut rally_bytes, self.exit_rally_point.x);
        push_f32(&mut rally_bytes, self.exit_rally_point.y);
        push_f32(&mut rally_bytes, self.exit_rally_point.z);
        state.insert("exit_rally_point".to_string(), rally_bytes);

        Ok(state)
    }

    /// Deserialize state for save/load
    pub fn load_state(&mut self, state: &HashMap<String, Vec<u8>>) -> GameResult<()> {
        self.base.load_state(state)?;

        fn read_f32(data: &[u8], offset: &mut usize) -> Option<f32> {
            if *offset + 4 > data.len() {
                return None;
            }
            let value = f32::from_le_bytes(data[*offset..*offset + 4].try_into().ok()?);
            *offset += 4;
            Some(value)
        }

        fn read_u32(data: &[u8], offset: &mut usize) -> Option<u32> {
            if *offset + 4 > data.len() {
                return None;
            }
            let value = u32::from_le_bytes(data[*offset..*offset + 4].try_into().ok()?);
            *offset += 4;
            Some(value)
        }

        if let Some(data) = state.get("garrison_points_in_use") {
            if data.len() >= 4 {
                self.garrison_points_in_use =
                    u32::from_le_bytes(data[0..4].try_into().map_err(|_| "Invalid data")?) as usize;
            }
        }

        if let Some(data) = state.get("original_team_id") {
            if data.len() >= 4 {
                let team_id = u32::from_le_bytes(data[0..4].try_into().map_err(|_| "Invalid data")?)
                    as crate::team::TeamID;
                if team_id != crate::team::TEAM_ID_INVALID {
                    if let Ok(factory) = crate::team::TheTeamFactory().lock() {
                        if let Some(team) = factory.find_team_by_id(team_id) {
                            self.original_team = Some(Arc::downgrade(&team));
                        }
                    }
                } else {
                    self.original_team = None;
                }
            }
        }

        if let Some(data) = state.get("hide_garrisoned_state") {
            self.hide_garrisoned_state_from_non_allies = data.first().copied().unwrap_or(0) != 0;
        }

        if let Some(data) = state.get("garrison_point_data") {
            let mut offset = 0usize;
            for point in &mut self.garrison_point_data {
                let object_id = read_u32(data, &mut offset).unwrap_or(INVALID_ID as u32);
                let target_id = read_u32(data, &mut offset).unwrap_or(INVALID_ID as u32);
                point.place_frame = read_u32(data, &mut offset).unwrap_or(0);
                point.last_effect_frame = read_u32(data, &mut offset).unwrap_or(0);
                let effect_id = read_u32(data, &mut offset).unwrap_or(0);

                point.object_id = if object_id == INVALID_ID as u32 {
                    INVALID_ID
                } else {
                    object_id as ObjectID
                };
                point.target_id = if target_id == INVALID_ID as u32 {
                    INVALID_ID
                } else {
                    target_id as ObjectID
                };
                point.effect_id = if effect_id == 0 {
                    None
                } else {
                    Some(effect_id)
                };
                point.effect = None;
            }
        }

        if let Some(data) = state.get("garrison_points") {
            let mut offset = 0usize;
            for condition in &mut self.garrison_points {
                for point in condition {
                    if let (Some(x), Some(y), Some(z)) = (
                        read_f32(data, &mut offset),
                        read_f32(data, &mut offset),
                        read_f32(data, &mut offset),
                    ) {
                        *point = Coord3D::new(x, y, z);
                    }
                }
            }
        }

        if let Some(data) = state.get("garrison_points_initialized") {
            self.garrison_points_initialized = data.first().copied().unwrap_or(0) != 0;
        }
        self.station_garrison_points_initialized = false;
        self.station_point_list.clear();

        if let Some(data) = state.get("rally_valid") {
            self.rally_valid = data.first().copied().unwrap_or(0) != 0;
        }
        if let Some(data) = state.get("exit_rally_point") {
            let mut offset = 0usize;
            if let (Some(x), Some(y), Some(z)) = (
                read_f32(data, &mut offset),
                read_f32(data, &mut offset),
                read_f32(data, &mut offset),
            ) {
                self.exit_rally_point = Coord3D::new(x, y, z);
            }
        }

        Ok(())
    }

    /// Post-process after loading to reconnect object/effect pointers
    pub fn load_post_process(&mut self) -> GameResult<()> {
        self.base.load_post_process()?;

        for point in &mut self.garrison_point_data {
            if point.object_id != INVALID_ID {
                let resolved = TheGameLogic::find_object_by_id(point.object_id).or_else(|| {
                    crate::object::registry::OBJECT_REGISTRY.get_object(point.object_id)
                });
                if resolved.is_none() {
                    return Err("GarrisonContain::load_post_process: missing object".into());
                }
            }

            if let Some(effect_id) = point.effect_id {
                if effect_id != 0 {
                    if let Some(client) = TheGameClient::get() {
                        let effect = client.get_drawable_arc(effect_id);
                        if effect.is_none() {
                            return Err("GarrisonContain::load_post_process: missing effect".into());
                        }
                        point.effect = effect;
                    }
                } else {
                    point.effect = None;
                }
            }
        }

        Ok(())
    }
}

impl Snapshotable for GarrisonContain {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::crc(&self.base, xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;

        Snapshotable::xfer(&mut self.base, xfer)?;

        let mut original_team_id = self
            .original_team
            .as_ref()
            .and_then(|team| team.upgrade())
            .and_then(|team| team.read().ok().map(|guard| guard.get_id()))
            .unwrap_or(crate::team::TEAM_ID_INVALID);
        xfer.xfer_unsigned_int(&mut original_team_id)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.original_team = if original_team_id != crate::team::TEAM_ID_INVALID {
                crate::team::TheTeamFactory()
                    .lock()
                    .ok()
                    .and_then(|factory| factory.find_team_by_id(original_team_id))
                    .map(|team| Arc::downgrade(&team))
            } else {
                None
            };
        }

        xfer.xfer_bool(&mut self.hide_garrisoned_state_from_non_allies)
            .map_err(|e| e.to_string())?;

        let mut point_data_count = MAX_GARRISON_POINTS as u16;
        xfer.xfer_unsigned_short(&mut point_data_count)
            .map_err(|e| e.to_string())?;
        for index in 0..usize::from(point_data_count).min(MAX_GARRISON_POINTS) {
            let point = &mut self.garrison_point_data[index];
            let mut object_id = point.object_id;
            xfer.xfer_object_id(&mut object_id)
                .map_err(|e| e.to_string())?;

            let mut target_id = point.target_id;
            xfer.xfer_object_id(&mut target_id)
                .map_err(|e| e.to_string())?;

            xfer.xfer_unsigned_int(&mut point.place_frame)
                .map_err(|e| e.to_string())?;
            xfer.xfer_unsigned_int(&mut point.last_effect_frame)
                .map_err(|e| e.to_string())?;

            let mut effect_id = point.effect_id.unwrap_or(0);
            xfer.xfer_drawable_id(&mut effect_id)
                .map_err(|e| e.to_string())?;

            if xfer.get_xfer_mode() == XferMode::Load {
                point.object_id = if object_id == INVALID_ID as u32 {
                    INVALID_ID
                } else {
                    object_id as ObjectID
                };
                point.target_id = if target_id == INVALID_ID as u32 {
                    INVALID_ID
                } else {
                    target_id as ObjectID
                };
                point.effect_id = if effect_id == 0 {
                    None
                } else {
                    Some(effect_id)
                };
                point.effect = None;
            }
        }

        let mut garrison_points_in_use = self.garrison_points_in_use as i32;
        xfer.xfer_int(&mut garrison_points_in_use)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.garrison_points_in_use = garrison_points_in_use.max(0) as usize;
        }

        for condition in &mut self.garrison_points {
            for point in condition {
                xfer.xfer_real(&mut point.x).map_err(|e| e.to_string())?;
                xfer.xfer_real(&mut point.y).map_err(|e| e.to_string())?;
                xfer.xfer_real(&mut point.z).map_err(|e| e.to_string())?;
            }
        }

        xfer.xfer_bool(&mut self.garrison_points_initialized)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.station_garrison_points_initialized = false;
            self.station_point_list.clear();
        }

        xfer.xfer_bool(&mut self.rally_valid)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.exit_rally_point.x)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.exit_rally_point.y)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.exit_rally_point.z)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        GarrisonContain::load_post_process(self).map_err(|e| e.to_string())
    }
}

impl ContainModuleInterface for GarrisonContain {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        if let Some(obj) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(obj_guard) = obj.read() {
                return self.is_valid_container_for(&*obj_guard, true);
            }
        }
        false
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.add_to_contain(object_id).map_err(|e| e.to_string())
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = match TheGameLogic::find_object_by_id(object_id) {
            Some(obj) => obj,
            None => return Ok(()),
        };
        self.remove_from_contain(
            obj.read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
            true,
        )
        .map_err(|e| e.to_string())
    }

    fn get_contained_objects(&self) -> &[ObjectID] {
        ContainModuleInterface::get_contained_objects(&self.base)
    }

    fn get_contained_count(&self) -> usize {
        ContainModuleInterface::get_contained_count(&self.base)
    }

    fn get_player_who_entered(&self) -> PlayerMaskType {
        self.base.get_player_who_entered()
    }

    fn get_max_capacity(&self) -> usize {
        let max = self.base.get_contain_max();
        if max < 0 {
            usize::MAX
        } else {
            max as usize
        }
    }

    fn snapshot_crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::crc(self, xfer)
    }

    fn snapshot_xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn snapshot_load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(self)
    }

    fn set_evac_disposition(&mut self, disposition: crate::common::UnsignedInt) {
        let mapped = match disposition {
            1 => EvacDisposition::ToLeft,
            2 => EvacDisposition::ToRight,
            3 => EvacDisposition::BurstFromCenter,
            _ => EvacDisposition::Invalid,
        };
        GarrisonContain::set_evac_disposition(self, mapped);
    }

    fn on_owner_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.on_object_created().map_err(|e| e.into())
    }

    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        GarrisonContain::update(self).map_err(|e| e.into())
    }

    fn on_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        GarrisonContain::on_damage(self, damage_info).map_err(|e| e.into())
    }

    fn on_body_damage_state_change(
        &mut self,
        damage_info: &DamageInfo,
        old_state: BodyDamageType,
        new_state: BodyDamageType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        GarrisonContain::on_body_damage_state_change(self, Some(damage_info), old_state, new_state)
            .map_err(|e| e.into())
    }

    fn on_die(
        &mut self,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_die(damage_info).map_err(|e| e.into())
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        self.is_valid_container_for(obj, check_capacity)
    }

    fn add_to_contain(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain_object(obj.get_id()).map_err(|e| e.into())
    }

    fn enable_load_sounds(
        &mut self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.enable_load_sounds(enabled);
        Ok(())
    }

    fn on_object_wants_to_enter_or_exit(
        &mut self,
        obj: &Object,
        want: ContainWant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_object_wants_to_enter_or_exit(obj, want);
        Ok(())
    }

    fn is_garrisonable(&self) -> bool {
        GarrisonContain::is_garrisonable(self)
    }

    fn is_bustable(&self) -> bool {
        GarrisonContain::is_bustable(self)
    }

    fn is_heal_contain(&self) -> bool {
        GarrisonContain::is_heal_contain(self)
    }

    fn is_immune_to_clear_building_attacks(&self) -> bool {
        GarrisonContain::is_immune_to_clear_building_attacks(self)
    }

    fn get_stealth_units_contained(&self) -> UnsignedInt {
        self.base.get_stealth_units_contained()
    }

    fn get_apparent_controlling_player(
        &self,
        observing_player: Option<&Player>,
    ) -> Option<Arc<RwLock<Player>>> {
        GarrisonContain::get_apparent_controlling_player(self, observing_player)
    }

    fn is_passenger_allowed_to_fire(&self, id: Option<ObjectID>) -> bool {
        GarrisonContain::is_passenger_allowed_to_fire(self, id)
    }

    fn passes_weapon_bonus_to_passengers(&self) -> bool {
        self.base.passes_weapon_bonus_to_passengers()
    }

    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        self.base.set_passenger_allowed_to_fire(allowed);
    }

    fn on_containing(
        &mut self,
        obj_id: ObjectID,
        was_selected: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        GarrisonContain::on_containing(self, obj_id, was_selected).map_err(|e| e.into())
    }

    fn on_removing(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        GarrisonContain::on_removing(self, obj_id).map_err(|e| e.into())
    }

    fn remove_all_contained(
        &mut self,
        expose_stealth: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        GarrisonContain::remove_all_contained(self, expose_stealth).map_err(|e| e.into())
    }

    fn harm_and_force_exit_all_contained(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        while let Some(object_id) = self.get_contained_objects().first().cloned() {
            let _ = self.release_object(object_id);
            if let Some(obj) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(mut guard) = obj.write() {
                    let _ = guard.attempt_damage(damage_info);
                }
            }
        }
        Ok(())
    }

    fn kill_all_contained(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        while let Some(object_id) = self.get_contained_objects().first().cloned() {
            let _ = self.release_object(object_id);
            if let Some(obj) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(mut guard) = obj.write() {
                    guard.kill(None, None);
                }
            }
        }
        Ok(())
    }

    fn is_displayed_on_control_bar(&self) -> bool {
        GarrisonContain::is_displayed_on_control_bar(self)
    }

    fn client_visible_contained_flash_as_selected(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .client_visible_contained_flash_as_selected()
            .map_err(|e| e.into())
    }

    fn is_enclosing_container_for(&self, _obj: &Object) -> bool {
        self.is_enclosing_container_for_any()
    }

    fn attempt_best_fire_point_position(
        &mut self,
        source_id: ObjectID,
        weapon: &Weapon,
        victim_id: ObjectID,
    ) -> bool {
        GarrisonContain::attempt_best_fire_point_position(self, source_id, weapon, victim_id)
    }

    fn attempt_best_fire_point_position_coord(
        &mut self,
        source_id: ObjectID,
        weapon: &Weapon,
        target_pos: &Coord3D,
    ) -> bool {
        GarrisonContain::attempt_best_fire_point_position_coord(self, source_id, weapon, target_pos)
    }
}

impl ContainerInterface for GarrisonContain {
    fn can_contain(&self, obj: &Object) -> bool {
        self.is_valid_container_for(obj, true)
    }

    fn add_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.add_to_contain(obj_id)
    }

    fn remove_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.remove_from_contain(obj_id, false)
    }

    fn get_usage(&self) -> (u32, u32) {
        let current = self.base.get_contain_count();
        let max = match self.base.get_contain_max() {
            super::CONTAIN_MAX_UNKNOWN => u32::MAX,
            value if value < 0 => u32::MAX,
            value => value as u32,
        };
        (current, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{DefaultThingTemplate, ObjectStatusMaskType, WeaponBonusConditionFlags};
    use crate::object::drawable::{DrawableExt, DrawableType};
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::player::{Player, ThePlayerList};
    use game_engine::common::system::{XferBlockSize, XferStatus};
    use std::io;

    struct RecordingXfer {
        bytes: Vec<u8>,
    }

    impl RecordingXfer {
        fn new() -> Self {
            Self { bytes: Vec::new() }
        }
    }

    impl Xfer for RecordingXfer {
        fn get_xfer_mode(&self) -> XferMode {
            XferMode::Save
        }

        fn get_identifier(&self) -> &str {
            "garrison-contain-test"
        }

        fn set_options(&mut self, _options: u32) {}

        fn clear_options(&mut self, _options: u32) {}

        fn get_options(&self) -> u32 {
            0
        }

        fn open(&mut self, _identifier: &str) -> Result<(), XferStatus> {
            Ok(())
        }

        fn close(&mut self) -> Result<(), XferStatus> {
            Ok(())
        }

        fn begin_block(&mut self) -> Result<XferBlockSize, XferStatus> {
            Ok(0)
        }

        fn end_block(&mut self) -> Result<(), XferStatus> {
            Ok(())
        }

        fn skip(&mut self, _data_size: i32) -> Result<(), XferStatus> {
            Ok(())
        }

        fn xfer_snapshot(
            &mut self,
            _snapshot: &mut game_engine::system::Snapshot,
        ) -> Result<(), XferStatus> {
            Ok(())
        }

        fn xfer_ascii_string(&mut self, _ascii_string_data: &mut String) -> io::Result<()> {
            Ok(())
        }

        fn xfer_unicode_string(&mut self, _unicode_string_data: &mut String) -> io::Result<()> {
            Ok(())
        }

        unsafe fn xfer_implementation(
            &mut self,
            data: *mut u8,
            data_size: usize,
        ) -> io::Result<()> {
            let bytes = unsafe { std::slice::from_raw_parts(data, data_size) };
            self.bytes.extend_from_slice(bytes);
            Ok(())
        }
    }

    #[test]
    fn test_garrison_module_data_defaults() {
        let data = GarrisonContainModuleData::default();
        assert!(!data.do_heal_objects);
        assert_eq!(data.frames_for_full_heal, 1.0);
        assert!(!data.mobile_garrison);
        assert!(!data.immune_to_clear_building_attacks);
        assert!(data.is_enclosing_container);
    }

    #[test]
    fn garrison_parse_from_config_preserves_cpp_fields() {
        let _lock = crate::test_sync::lock();

        let mut data = GarrisonContainModuleData::default();
        data.parse_from_config(
            "MobileGarrison = Yes\n\
             HealObjects = Yes\n\
             TimeForFullHeal = 2s\n\
             InitialRoster = GLAInfantryRebel 3\n\
             ImmuneToClearBuildingAttacks = Yes\n\
             IsEnclosingContainer = No\n\
             End\n",
        )
        .expect("garrison config parses");

        assert!(data.mobile_garrison);
        assert!(data.do_heal_objects);
        assert!((data.frames_for_full_heal - 60.0).abs() < 0.001);
        assert_eq!(data.initial_roster.template_name, "GLAInfantryRebel");
        assert_eq!(data.initial_roster.count, 3);
        assert!(data.immune_to_clear_building_attacks);
        assert!(!data.is_enclosing_container);
    }

    #[test]
    fn test_garrison_point_data_default() {
        let point = GarrisonPointData::default();
        assert_eq!(point.object_id, INVALID_ID);
        assert_eq!(point.object_id, INVALID_ID);
        assert_eq!(point.target_id, INVALID_ID);
        assert_eq!(point.place_frame, 0);
        assert_eq!(point.last_effect_frame, 0);
        assert!(point.effect.is_none());
        assert!(point.effect_id.is_none());
    }

    #[test]
    fn test_initial_roster_default() {
        let roster = InitialRoster::default();
        assert_eq!(roster.template_name, "");
        assert_eq!(roster.count, 0);
    }

    #[test]
    fn trait_snapshot_xfer_writes_garrison_version_and_open_contain_state_like_cpp() {
        let mut contain = GarrisonContain::new(Weak::new(), &GarrisonContainModuleData::default())
            .expect("garrison contain");
        contain.hide_garrisoned_state_from_non_allies = true;
        contain.garrison_points_in_use = 2;
        contain.rally_valid = true;
        contain.exit_rally_point = Coord3D::new(11.0, 22.0, 3.0);

        let mut xfer = RecordingXfer::new();
        ContainModuleInterface::snapshot_xfer(&mut contain, &mut xfer)
            .expect("garrison snapshot xfer");

        assert_eq!(xfer.bytes[0], 1, "GarrisonContain xfer version");
        assert_eq!(xfer.bytes[1], 2, "delegated OpenContain xfer version");
        assert!(
            xfer.bytes.len() > 1,
            "trait snapshot hook must not fall back to no-op"
        );
    }

    #[test]
    fn test_garrison_point_condition_values() {
        assert_eq!(GarrisonPointCondition::Pristine as usize, 0);
        assert_eq!(GarrisonPointCondition::Damaged as usize, 1);
        assert_eq!(GarrisonPointCondition::ReallyDamaged as usize, 2);
    }

    #[test]
    fn test_max_garrison_points_constant() {
        assert_eq!(MAX_GARRISON_POINTS, 40);
    }

    #[test]
    fn test_max_garrison_point_conditions_constant() {
        assert_eq!(MAX_GARRISON_POINT_CONDITIONS, 3);
    }

    #[test]
    fn test_muzzle_flash_lifetime_constant() {
        // MUZZLE_FLASH_LIFETIME should be approximately 4 frames (30/7)
        assert_eq!(MUZZLE_FLASH_LIFETIME, 30 / 7);
        assert_eq!(MUZZLE_FLASH_LIFETIME, 4);
    }

    #[test]
    fn test_evac_disposition_variants() {
        let disp = EvacDisposition::Invalid;
        assert!(matches!(disp, EvacDisposition::Invalid));

        let disp = EvacDisposition::ToLeft;
        assert!(matches!(disp, EvacDisposition::ToLeft));
    }

    #[test]
    fn garrison_defaults_to_cpp_burst_evac_disposition() {
        let data = GarrisonContainModuleData::default();
        let contain = GarrisonContain::new(Weak::new(), &data).expect("garrison constructs");

        assert!(matches!(
            contain.evac_disposition,
            EvacDisposition::BurstFromCenter
        ));
    }

    #[test]
    fn garrison_save_state_omits_station_garrison_state_like_cpp() {
        let data = GarrisonContainModuleData::default();
        let contain = GarrisonContain::new(Weak::new(), &data).expect("garrison constructs");
        let state = contain.save_state().expect("garrison saves");

        assert!(!state.contains_key("station_garrison_points_initialized"));
        assert!(!state.contains_key("station_points"));
    }

    #[test]
    fn load_station_garrison_points_leaves_empty_list_without_station_bones_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("StationlessGarrisonOwner", 96007, 0);
        let drawable = Arc::new(RwLock::new(Drawable::new(
            97007,
            96007,
            "StationlessGarrisonModel".to_string(),
            DrawableType::Static,
        )));
        owner
            .write()
            .expect("owner write")
            .set_drawable(Some(drawable));
        let data = GarrisonContainModuleData {
            is_enclosing_container: false,
            ..Default::default()
        };
        let mut contain =
            GarrisonContain::new(Arc::downgrade(&owner), &data).expect("garrison constructs");

        contain
            .load_station_garrison_points()
            .expect("station point load succeeds");

        assert!(contain.station_garrison_points_initialized);
        assert!(
            contain.station_point_list.is_empty(),
            "C++ pushes only returned STATION bones and leaves the list empty when none exist"
        );
        cleanup_objects(&[96007]);
    }

    #[test]
    fn test_clear_building_attack_immunity() {
        let data = GarrisonContainModuleData {
            immune_to_clear_building_attacks: true,
            ..Default::default()
        };
        assert!(data.immune_to_clear_building_attacks);

        let data = GarrisonContainModuleData {
            immune_to_clear_building_attacks: false,
            ..Default::default()
        };
        assert!(!data.immune_to_clear_building_attacks);
    }

    fn reset_players() {
        let mut list = ThePlayerList().write().expect("player list write");
        list.clear();
        list.add_player(Arc::new(RwLock::new(Player::new(0))));
    }

    fn owned_object(name: &str, id: ObjectID, player_index: u32) -> Arc<RwLock<Object>> {
        let team = Arc::new(RwLock::new(Team::new(
            format!("{name}Team").into(),
            id + 10_000,
        )));
        team.write()
            .expect("team write")
            .set_controlling_player_id(Some(player_index));
        let template = Arc::new(DefaultThingTemplate::new(name.to_string()));
        Object::new_with_id(template, id, ObjectStatusMaskType::none(), Some(team))
            .expect("owned test object")
    }

    fn infantry(name: &str, id: ObjectID, player_index: u32) -> Arc<RwLock<Object>> {
        let team = Arc::new(RwLock::new(Team::new(
            format!("{name}Team").into(),
            id + 10_000,
        )));
        team.write()
            .expect("team write")
            .set_controlling_player_id(Some(player_index));
        let mut template = DefaultThingTemplate::new(name.to_string());
        let mut fields = HashMap::new();
        fields.insert("KindOf".to_string(), "INFANTRY".to_string());
        template.parse_object_fields_from_ini(&fields);
        Object::new_with_id(
            Arc::new(template),
            id,
            ObjectStatusMaskType::none(),
            Some(team),
        )
        .expect("infantry test object")
    }

    fn cleanup_objects(ids: &[ObjectID]) {
        for id in ids {
            OBJECT_REGISTRY.unregister_object(*id);
        }
        ThePlayerList().write().expect("player list write").clear();
    }

    #[test]
    fn trait_containment_uses_garrison_lifecycle_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("GarrisonOwner", 96001, 0);
        let passenger = infantry("GarrisonPassenger", 96002, 0);
        let mut contain = GarrisonContain::new(
            Arc::downgrade(&owner),
            &GarrisonContainModuleData::default(),
        )
        .expect("garrison contain");

        ContainModuleInterface::contain_object(&mut contain, 96002).expect("contain passenger");

        assert_eq!(ContainModuleInterface::get_contained_count(&contain), 1);
        assert_eq!(
            passenger.read().expect("passenger read").get_contained_by(),
            Some(96001)
        );
        assert!(passenger
            .read()
            .expect("passenger read")
            .is_disabled_by_type(DisabledType::Held));
        assert!(passenger
            .read()
            .expect("passenger read")
            .get_weapon_bonus_condition()
            .contains(WeaponBonusConditionFlags::GARRISONED));
        assert!(owner
            .read()
            .expect("owner read")
            .test_status(ObjectStatusTypes::CanAttack));

        cleanup_objects(&[96001, 96002]);
    }

    #[test]
    fn trait_release_clears_last_garrison_occupant_state_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("GarrisonReleaseOwner", 96003, 0);
        let passenger = infantry("GarrisonReleasePassenger", 96004, 0);
        let mut contain = GarrisonContain::new(
            Arc::downgrade(&owner),
            &GarrisonContainModuleData::default(),
        )
        .expect("garrison contain");

        ContainModuleInterface::contain_object(&mut contain, 96004).expect("contain passenger");
        contain.hide_garrisoned_state_from_non_allies = true;
        ContainModuleInterface::release_object(&mut contain, 96004).expect("release passenger");

        assert_eq!(ContainModuleInterface::get_contained_count(&contain), 0);
        assert_eq!(
            passenger.read().expect("passenger read").get_contained_by(),
            None
        );
        assert!(!passenger
            .read()
            .expect("passenger read")
            .is_disabled_by_type(DisabledType::Held));
        assert!(!passenger
            .read()
            .expect("passenger read")
            .get_weapon_bonus_condition()
            .contains(WeaponBonusConditionFlags::GARRISONED));
        assert!(!owner
            .read()
            .expect("owner read")
            .test_status(ObjectStatusTypes::CanAttack));
        assert!(!contain.hide_garrisoned_state_from_non_allies);

        cleanup_objects(&[96003, 96004]);
    }

    #[test]
    fn mobile_garrison_moves_every_contained_object_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("MobileGarrisonOwner", 96005, 0);
        let passenger = infantry("MobileGarrisonPassenger", 96006, 0);
        let data = GarrisonContainModuleData {
            mobile_garrison: true,
            ..Default::default()
        };
        let mut contain =
            GarrisonContain::new(Arc::downgrade(&owner), &data).expect("mobile garrison contain");

        ContainModuleInterface::contain_object(&mut contain, 96006).expect("contain passenger");
        let new_pos = Coord3D::new(123.0, 456.0, 7.0);
        owner
            .write()
            .expect("owner write")
            .set_position(&new_pos)
            .expect("set owner position");
        contain.move_objects_with_me().expect("move occupants");

        assert_eq!(
            *passenger.read().expect("passenger read").get_position(),
            new_pos
        );

        cleanup_objects(&[96005, 96006]);
    }
}
