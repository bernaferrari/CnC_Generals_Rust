//! TensileFormationUpdate - Springy formation motion (avalanche behavior)
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::ai::integration::with_ai_integration_mut;
use crate::common::xfer::XferExt;
use crate::common::GameLogicRandomValueReal;
use crate::common::{
    AsciiString, BodyDamageType, Bool, Coord3D, ICoord2D, KindOf, ModuleData, ObjectID, Real,
};
use crate::helpers::{TheAudio, TheGameLogic, ThePartitionManager, TheTerrainLogic};
use crate::modules::{
    BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_template::AudioEventRts;
use crate::object::{Object as GameObject, INVALID_ID as OBJECT_INVALID_ID};
use crate::path::{grid_to_world, world_to_grid, PathfindLayerEnum, PATHFIND_CELL_SIZE_F};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

const LINK_COUNT: usize = 4;

#[derive(Clone, Debug)]
pub struct TensileFormationUpdateModuleData {
    pub base: BehaviorModuleData,
    pub enabled: Bool,
    pub crack_sound: AudioEventRts,
}

impl Default for TensileFormationUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            enabled: false,
            crack_sound: AudioEventRts::default(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(TensileFormationUpdateModuleData, base);

impl TensileFormationUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, TENSILE_FORMATION_UPDATE_FIELDS)
    }
}

fn parse_enabled(
    _ini: &mut INI,
    data: &mut TensileFormationUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.enabled = INI::parse_bool(token)?;
    Ok(())
}

fn parse_crack_sound(
    _ini: &mut INI,
    data: &mut TensileFormationUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    if token.eq_ignore_ascii_case("NONE") {
        data.crack_sound = AudioEventRts::default();
    } else {
        data.crack_sound = AudioEventRts::new(*token);
    }
    Ok(())
}

const TENSILE_FORMATION_UPDATE_FIELDS: &[FieldParse<TensileFormationUpdateModuleData>] = &[
    FieldParse {
        token: "Enabled",
        parse: parse_enabled,
    },
    FieldParse {
        token: "CrackSound",
        parse: parse_crack_sound,
    },
];

#[derive(Clone, Copy, Debug)]
struct TensileFormationLink {
    id: ObjectID,
    tensor: Coord3D,
}

impl Default for TensileFormationLink {
    fn default() -> Self {
        Self {
            id: OBJECT_INVALID_ID,
            tensor: Coord3D::ZERO,
        }
    }
}

#[allow(dead_code)]
pub struct TensileFormationUpdate {
    object: Weak<RwLock<GameObject>>,
    #[allow(dead_code)]
    module_data: Arc<TensileFormationUpdateModuleData>,
    enabled: Bool,
    crack_sound: AudioEventRts,
    crack_sound_handle: Option<u32>,
    inertia: Coord3D,
    links_inited: Bool,
    motionless_counter: u32,
    life: u32,
    lowest_slide_elevation: Real,
    links: [TensileFormationLink; LINK_COUNT],
}

impl TensileFormationUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .as_any()
            .downcast_ref::<TensileFormationUpdateModuleData>()
            .ok_or("Invalid module data")?;

        let owner_id = object.read().map(|obj| obj.get_id()).unwrap_or(0);
        let mut crack_sound = specific_data.crack_sound.clone();
        crack_sound.set_object_id(owner_id);

        let instance = Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            enabled: specific_data.enabled,
            crack_sound,
            crack_sound_handle: None,
            inertia: Coord3D::ZERO,
            links_inited: false,
            motionless_counter: 0,
            life: 0,
            lowest_slide_elevation: 255.0,
            links: [TensileFormationLink::default(); LINK_COUNT],
        };

        instance.set_pathfinding_wall(true);

        Ok(instance)
    }

    fn set_enabled(&mut self, enabled: Bool) {
        self.enabled = enabled;
    }

    fn set_pathfinding_wall(&self, enable: Bool) {
        let Some(object_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(obj_guard) = object_arc.read() else {
            return;
        };
        let pos = *obj_guard.get_position();
        let radius = obj_guard.get_geometry_info().get_bounding_circle_radius();
        let object_id = obj_guard.get_id();
        drop(obj_guard);

        let positions = Self::pathfinding_footprint_positions(&pos, radius);
        let _ = with_ai_integration_mut(|manager| {
            if enable {
                manager.add_pathfinding_obstacle(object_id, &positions, false)
            } else {
                manager.remove_pathfinding_obstacle(object_id, &positions)
            }
        });
    }

    fn pathfinding_footprint_positions(pos: &Coord3D, radius: Real) -> Vec<Coord3D> {
        let center = world_to_grid(pos);
        let cell_radius = (radius / PATHFIND_CELL_SIZE_F).ceil() as i32;
        let mut positions = Vec::new();

        for dy in -cell_radius..=cell_radius {
            for dx in -cell_radius..=cell_radius {
                let cell = ICoord2D::new(center.x + dx, center.y + dy);
                let world = grid_to_world(&cell, PathfindLayerEnum::Ground);
                let delta = Coord3D::new(world.x - pos.x, world.y - pos.y, 0.0);
                if delta.length_squared() <= radius * radius {
                    positions.push(world);
                }
            }
        }

        if positions.is_empty() {
            positions.push(*pos);
        }

        positions
    }

    fn init_links(&mut self) {
        self.links_inited = true;

        let Some(object_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(obj_guard) = object_arc.read() else {
            return;
        };
        let my_pos = *obj_guard.get_position();
        let Some(partition) = ThePartitionManager::get() else {
            return;
        };

        let query_radius = 1000.0;
        let mut closest_distance = 99999.9_f32;
        for id in partition.get_objects_in_range_boundary_3d_from_object(&obj_guard, query_radius) {
            let Some(other_arc) = TheGameLogic::find_object_by_id(id) else {
                continue;
            };
            let Ok(other) = other_arc.read() else {
                continue;
            };
            if other.find_update_module("TensileFormationUpdate").is_none() {
                continue;
            }
            let delta = *other.get_position() - my_pos;
            let this_distance = delta.length();
            if this_distance < closest_distance {
                closest_distance = this_distance;
                for idx in (1..LINK_COUNT).rev() {
                    self.links[idx] = self.links[idx - 1];
                }
                self.links[0] = TensileFormationLink { id, tensor: delta };
            }
        }

        drop(obj_guard);
        if let Ok(mut obj_guard) = object_arc.write() {
            let angle = GameLogicRandomValueReal(-std::f32::consts::PI, std::f32::consts::PI);
            let _ = obj_guard.set_orientation(angle);
        };
    }

    fn propagate_dislodgement(&self) {
        let Some(object_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(obj_guard) = object_arc.read() else {
            return;
        };
        let Some(partition) = ThePartitionManager::get() else {
            return;
        };

        let query_radius = 100.0;
        for id in partition.get_objects_in_range_boundary_3d_from_object(&obj_guard, query_radius) {
            let Some(other_arc) = TheGameLogic::find_object_by_id(id) else {
                continue;
            };
            let Ok(other) = other_arc.write() else {
                continue;
            };
            if other.find_update_module("TensileFormationUpdate").is_none() {
                continue;
            }
            if let Some(body) = other.get_body_module() {
                if let Ok(mut body_guard) = body.lock() {
                    let _ = body_guard.set_damage_state(BodyDamageType::Damaged);
                }
            }
        }

        for link in self.links.iter() {
            if link.id == OBJECT_INVALID_ID {
                continue;
            }
            let Some(other_arc) = TheGameLogic::find_object_by_id(link.id) else {
                continue;
            };
            let Ok(other) = other_arc.write() else {
                continue;
            };
            if let Some(body) = other.get_body_module() {
                if let Ok(mut body_guard) = body.lock() {
                    let _ = body_guard.set_damage_state(BodyDamageType::Damaged);
                }
            }
        }
    }
}

impl UpdateModuleInterface for TensileFormationUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if !self.links_inited {
            self.init_links();
        }

        if !self.enabled {
            let Some(object_arc) = self.object.upgrade() else {
                return UpdateSleepTime::Frames(30);
            };
            let Ok(obj_guard) = object_arc.read() else {
                return UpdateSleepTime::Frames(30);
            };
            let Some(body_arc) = obj_guard.get_body_module() else {
                return UpdateSleepTime::Frames(30);
            };
            let Ok(body_guard) = body_arc.lock() else {
                return UpdateSleepTime::Frames(30);
            };
            let damaged = matches!(
                body_guard.get_damage_state(),
                BodyDamageType::Damaged | BodyDamageType::ReallyDamaged | BodyDamageType::Rubble
            );
            if damaged {
                drop(body_guard);
                self.set_enabled(true);
                self.set_pathfinding_wall(false);
                if !self.crack_sound.is_currently_playing() {
                    if let Some(audio) = TheAudio::get() {
                        let handle = audio.add_audio_event(&self.crack_sound);
                        self.crack_sound_handle = Some(handle);
                        self.crack_sound.set_playing_handle(handle);
                    }
                }
            } else {
                return UpdateSleepTime::Frames(30);
            }
        }

        let Some(object_arc) = self.object.upgrade() else {
            return UPDATE_SLEEP_NONE;
        };
        let drawable = {
            let Ok(obj_guard) = object_arc.read() else {
                return UPDATE_SLEEP_NONE;
            };
            obj_guard.get_drawable()
        };
        if drawable.is_none() {
            return UPDATE_SLEEP_NONE;
        }

        self.life += 1;
        if self.life > 300 {
            if let Some(drawable) = drawable {
                if let Ok(mut draw_guard) = drawable.write() {
                    draw_guard
                        .clear_model_condition_state(crate::common::ModelConditionFlags::MOVING);
                    draw_guard
                        .clear_model_condition_state(crate::common::ModelConditionFlags::FREEFALL);
                    draw_guard.clear_model_condition_state(
                        crate::common::ModelConditionFlags::POST_COLLAPSE,
                    );
                }
            }
            if let Ok(obj_guard) = object_arc.write() {
                if let Some(body) = obj_guard.get_body_module() {
                    if let Ok(mut body_guard) = body.lock() {
                        let _ = body_guard.set_damage_state(BodyDamageType::Rubble);
                    }
                }
            }
            self.set_pathfinding_wall(true);
            return UpdateSleepTime::Forever;
        }

        if self.life % 30 == 29 {
            self.propagate_dislodgement();
        }

        let (pos, major_radius, _object_id) = {
            let Ok(obj_guard) = object_arc.read() else {
                return UPDATE_SLEEP_NONE;
            };
            let pos = *obj_guard.get_position();
            let radius = obj_guard.get_geometry_info().get_major_radius();
            let object_id = obj_guard.get_id();
            (pos, radius, object_id)
        };

        let Some(terrain) = TheTerrainLogic::get() else {
            return UPDATE_SLEEP_NONE;
        };

        let mut normal = Coord3D::new(0.0, 0.0, 1.0);
        let _ = terrain.get_ground_height(pos.x, pos.y, Some(&mut normal));
        let steepness = 1.0 - normal.z;
        let mut slope = Coord3D::new(normal.x, normal.y, 0.0);
        slope *= 0.3 + steepness;
        self.inertia += slope;
        self.inertia *= 0.95;

        let mut new_pos = Coord3D::new(pos.x + self.inertia.x, pos.y + self.inertia.y, pos.z);
        new_pos.z = terrain.get_ground_height(new_pos.x, new_pos.y, None);

        if let Some(partition) = ThePartitionManager::get() {
            let mut closest_id = None;
            let mut closest_dist_sqr = major_radius * major_radius + 1.0;
            for id in partition.get_objects_in_range(&new_pos, major_radius) {
                let Some(obj_arc) = OBJECT_REGISTRY.get_object(id) else {
                    continue;
                };
                let Ok(obj) = obj_arc.read() else {
                    continue;
                };
                let obj_pos = obj.get_position();
                let dx = obj_pos.x - new_pos.x;
                let dy = obj_pos.y - new_pos.y;
                let dist_sqr = dx * dx + dy * dy;
                if dist_sqr < closest_dist_sqr {
                    closest_dist_sqr = dist_sqr;
                    closest_id = Some(id);
                }
            }

            if let Some(tree_id) = closest_id {
                if let Some(tree_arc) = TheGameLogic::find_object_by_id(tree_id) {
                    if let Ok(mut tree) = tree_arc.write() {
                        if tree.is_kind_of(KindOf::Shrubbery) {
                            tree.topple(
                                &self.inertia,
                                self.inertia.length(),
                                crate::object::behavior::topple_update::TOPPLE_OPTIONS_NO_BOUNCE,
                            );
                        }
                    }
                }
            }
        }

        let mut tensor_sum = Coord3D::ZERO;
        for link in self.links.iter() {
            if link.id == OBJECT_INVALID_ID {
                continue;
            }
            let Some(other_arc) = TheGameLogic::find_object_by_id(link.id) else {
                continue;
            };
            let Ok(other) = other_arc.read() else {
                continue;
            };
            let desired_pos = *other.get_position() - link.tensor;
            new_pos.x = new_pos.x * 0.93 + desired_pos.x * 0.07;
            new_pos.y = new_pos.y * 0.93 + desired_pos.y * 0.07;
            let ground = terrain.get_ground_height(new_pos.x, new_pos.y, None);
            new_pos.z = self.lowest_slide_elevation.min(ground);

            tensor_sum += link.tensor.normalize_or_zero();
        }

        let _tensor_sum = tensor_sum.normalize_or_zero();
        let _inertia_normal = self.inertia.normalize_or_zero();

        if let Some(drawable) = drawable {
            if let Ok(mut draw_guard) = drawable.write() {
                draw_guard
                    .set_model_condition_state(crate::common::ModelConditionFlags::POST_COLLAPSE);
                if self.life < 200 {
                    draw_guard
                        .set_model_condition_state(crate::common::ModelConditionFlags::MOVING);
                } else {
                    draw_guard
                        .clear_model_condition_state(crate::common::ModelConditionFlags::MOVING);
                }
                if (pos.z - new_pos.z).abs() > 0.2 && self.life < 100 {
                    draw_guard
                        .set_model_condition_state(crate::common::ModelConditionFlags::FREEFALL);
                } else {
                    draw_guard
                        .clear_model_condition_state(crate::common::ModelConditionFlags::FREEFALL);
                }
            }
        }

        self.lowest_slide_elevation = new_pos.z;
        if let Ok(mut obj_guard) = object_arc.write() {
            let _ = obj_guard.set_position(&new_pos);
        }

        UPDATE_SLEEP_NONE
    }
}

impl BehaviorModuleInterface for TensileFormationUpdate {
    fn get_module_name(&self) -> &'static str {
        "TensileFormationUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for TensileFormationUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer.xfer_bool(&mut self.enabled)
            .map_err(|e| format!("Failed to xfer enabled: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes TensileFormationUpdate through the common Module trait.
pub struct TensileFormationUpdateModule {
    behavior: TensileFormationUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<TensileFormationUpdateModuleData>,
}

impl TensileFormationUpdateModule {
    pub fn new(
        behavior: TensileFormationUpdate,
        module_name: &AsciiString,
        module_data: Arc<TensileFormationUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut TensileFormationUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for TensileFormationUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

impl Module for TensileFormationUpdateModule {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }
}

pub struct TensileFormationUpdateFactory;
impl TensileFormationUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(TensileFormationUpdate::new(thing, module_data)?))
    }
}
