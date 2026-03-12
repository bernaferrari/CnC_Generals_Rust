//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BridgeBehavior.cpp`.
//!
//! BridgeBehavior - Rust conversion of C++ BridgeBehavior
//!
//! Behavior module for bridges - handles bridge construction, repair, and destruction.
//! Author: Colin Day, July 2002 (C++ version)
//! Rust conversion: 2025

use std::any::Any;
use std::array;
use std::f32::consts::TAU;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::ai::{pathfinding_system::PathfindLayerEnum as AiPathfindLayerEnum, THE_AI};
use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, AudioEventRTS, BehaviorModuleData, Bool, Bridge, Coord3D, FXList, Int, KindOf,
    ObjectCreationList, ObjectID, PathfindLayerEnum, Real, Team, TerrainRoadType, UnsignedInt,
    XferVersion,
};
use crate::damage::{BodyDamageType, DamageInfo};
use crate::helpers::{
    get_game_logic_random_value_real, TheFXListStore, TheGameLogic, TheObjectCreationListStore,
    ThePartitionManager, TheRadar, TheThingFactory,
};
use crate::modules::{
    BehaviorModuleInterface, DamageModuleInterface, DieModuleInterface, PhysicsBehaviorExt,
    UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::{
    behavior::bridge_tower_behavior::BridgeTowerBehaviorModule, drawable::DrawableExt,
    registry::OBJECT_REGISTRY, Object as GameObject, INVALID_ID as OBJECT_INVALID_ID,
};
use crate::terrain::THE_TERRAIN_LOGIC;
use game_engine::ascii_string::AsciiString as EngineAsciiString;
use game_engine::common::ini::ini_terrain_bridge::IniTerrainBridge;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, XferMode};
use game_engine::common::thing::module::{
    Module as EngineModule, ModuleData as ThingModuleData, NameKeyType, Thing as ModuleThing,
};
use game_engine::system::Xfer as EngineXfer;
use log::warn;

use super::behavior_module::{
    BridgeBehaviorInterface, BridgeScaffoldBehaviorInterface, BridgeTowerBehaviorInterface,
    BridgeTowerType, ScaffoldTargetMotion,
};

// Constants
const BRIDGE_MAX_TOWERS: usize = 4;
const MAX_BRIDGE_BODY_FX: usize = 3;
const BODYDAMAGETYPE_COUNT: usize = 5;
const BODY_PRISTINE: usize = 0;

fn terrain_layer_to_logic_layer(layer: crate::path::PathfindLayerEnum) -> PathfindLayerEnum {
    match layer {
        crate::path::PathfindLayerEnum::Ground => PathfindLayerEnum::Ground,
        crate::path::PathfindLayerEnum::Top => PathfindLayerEnum::Top,
        crate::path::PathfindLayerEnum::Bridge1 => PathfindLayerEnum::Bridge1,
        crate::path::PathfindLayerEnum::Bridge2 => PathfindLayerEnum::Bridge2,
        crate::path::PathfindLayerEnum::Bridge3 => PathfindLayerEnum::Bridge3,
        crate::path::PathfindLayerEnum::Bridge4 => PathfindLayerEnum::Bridge4,
        crate::path::PathfindLayerEnum::Wall => PathfindLayerEnum::Wall,
        crate::path::PathfindLayerEnum::Invalid | crate::path::PathfindLayerEnum::Last => {
            PathfindLayerEnum::Ground
        }
    }
}

/// Time and location information for bridge effects
#[derive(Debug, Clone)]
pub struct TimeAndLocationInfo {
    pub delay: UnsignedInt,
    pub bone_name: AsciiString,
}

impl TimeAndLocationInfo {
    pub fn new() -> Self {
        Self {
            delay: 0,
            bone_name: AsciiString::from(""),
        }
    }
}

impl Default for TimeAndLocationInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Bridge FX information
#[derive(Debug, Clone)]
pub struct BridgeFXInfo {
    pub fx_name: Option<AsciiString>,
    pub fx: Option<Arc<FXList>>,
    pub time_and_location_info: TimeAndLocationInfo,
}

impl BridgeFXInfo {
    pub fn new() -> Self {
        Self {
            fx_name: None,
            fx: None,
            time_and_location_info: TimeAndLocationInfo::new(),
        }
    }
}

/// Bridge OCL information
#[derive(Debug, Clone)]
pub struct BridgeOCLInfo {
    pub ocl_name: Option<AsciiString>,
    pub ocl: Option<Arc<ObjectCreationList>>,
    pub time_and_location_info: TimeAndLocationInfo,
}

impl BridgeOCLInfo {
    pub fn new() -> Self {
        Self {
            ocl_name: None,
            ocl: None,
            time_and_location_info: TimeAndLocationInfo::new(),
        }
    }
}

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Option<&'a str> {
    tokens.iter().copied().find(|token| *token != "=")
}

fn parse_lateral_scaffold_speed_field(
    _ini: &mut INI,
    data: &mut BridgeBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.lateral_scaffold_speed = INI::parse_real(value)?;
    Ok(())
}

fn parse_vertical_scaffold_speed_field(
    _ini: &mut INI,
    data: &mut BridgeBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.vertical_scaffold_speed = INI::parse_real(value)?;
    Ok(())
}

fn parse_bridge_fx_field(
    _ini: &mut INI,
    data: &mut BridgeBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let mut fx_info = BridgeFXInfo::new();
    let mut iter = tokens.iter().copied().peekable();

    while let Some(token) = iter.next() {
        if token == "=" {
            continue;
        }

        if let Some(name) = token.strip_prefix("FX:") {
            fx_info.fx_name = Some(AsciiString::from(name));
            continue;
        }

        if token.eq_ignore_ascii_case("FX") {
            let next = iter.next().ok_or(INIError::InvalidData)?;
            let stripped = next.strip_prefix(':').unwrap_or(next);
            fx_info.fx_name = Some(AsciiString::from(stripped));
            continue;
        }

        if token.eq_ignore_ascii_case("Delay:") {
            let value = iter.next().ok_or(INIError::InvalidData)?;
            fx_info.time_and_location_info.delay = INI::parse_unsigned_int(value)?;
            continue;
        }

        if let Some(value) = token.strip_prefix("Delay:") {
            let delay_value = if value.is_empty() {
                iter.next().ok_or(INIError::InvalidData)?
            } else {
                value
            };
            fx_info.time_and_location_info.delay = INI::parse_unsigned_int(delay_value)?;
            continue;
        }

        if token.eq_ignore_ascii_case("Bone:") {
            let value = iter.next().ok_or(INIError::InvalidData)?;
            fx_info.time_and_location_info.bone_name = AsciiString::from(value);
            continue;
        }

        if let Some(value) = token.strip_prefix("Bone:") {
            let bone_value = if value.is_empty() {
                iter.next().ok_or(INIError::InvalidData)?
            } else {
                value
            };
            fx_info.time_and_location_info.bone_name = AsciiString::from(bone_value);
            continue;
        }
    }

    if fx_info.fx_name.is_none() {
        return Err(INIError::InvalidData);
    }

    data.fx.push(fx_info);
    Ok(())
}

fn point_inside_area_2d(point: &Coord3D, polygon: &[Coord3D]) -> bool {
    if polygon.len() < 3 {
        return false;
    }

    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let xi = polygon[i].x;
        let yi = polygon[i].y;
        let xj = polygon[j].x;
        let yj = polygon[j].y;
        let intersects = ((yi > point.y) != (yj > point.y))
            && (point.x < (xj - xi) * (point.y - yi) / (yj - yi + f32::EPSILON) + xi);
        if intersects {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn parse_bridge_ocl_field(
    _ini: &mut INI,
    data: &mut BridgeBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let mut ocl_info = BridgeOCLInfo::new();
    let mut iter = tokens.iter().copied().peekable();

    while let Some(token) = iter.next() {
        if token == "=" {
            continue;
        }

        if let Some(name) = token.strip_prefix("OCL:") {
            ocl_info.ocl_name = Some(AsciiString::from(name));
            continue;
        }

        if token.eq_ignore_ascii_case("OCL") {
            let next = iter.next().ok_or(INIError::InvalidData)?;
            let stripped = next.strip_prefix(':').unwrap_or(next);
            ocl_info.ocl_name = Some(AsciiString::from(stripped));
            continue;
        }

        if token.eq_ignore_ascii_case("Delay:") {
            let value = iter.next().ok_or(INIError::InvalidData)?;
            ocl_info.time_and_location_info.delay = INI::parse_unsigned_int(value)?;
            continue;
        }

        if let Some(value) = token.strip_prefix("Delay:") {
            let delay_value = if value.is_empty() {
                iter.next().ok_or(INIError::InvalidData)?
            } else {
                value
            };
            ocl_info.time_and_location_info.delay = INI::parse_unsigned_int(delay_value)?;
            continue;
        }

        if token.eq_ignore_ascii_case("Bone:") {
            let value = iter.next().ok_or(INIError::InvalidData)?;
            ocl_info.time_and_location_info.bone_name = AsciiString::from(value);
            continue;
        }

        if let Some(value) = token.strip_prefix("Bone:") {
            let bone_value = if value.is_empty() {
                iter.next().ok_or(INIError::InvalidData)?
            } else {
                value
            };
            ocl_info.time_and_location_info.bone_name = AsciiString::from(bone_value);
            continue;
        }
    }

    if ocl_info.ocl_name.is_none() {
        return Err(INIError::InvalidData);
    }

    data.ocl.push(ocl_info);
    Ok(())
}

const BRIDGE_BEHAVIOR_FIELDS: &[FieldParse<BridgeBehaviorModuleData>] = &[
    FieldParse {
        token: "LateralScaffoldSpeed",
        parse: parse_lateral_scaffold_speed_field,
    },
    FieldParse {
        token: "VerticalScaffoldSpeed",
        parse: parse_vertical_scaffold_speed_field,
    },
    FieldParse {
        token: "BridgeDieFX",
        parse: parse_bridge_fx_field,
    },
    FieldParse {
        token: "BridgeDieOCL",
        parse: parse_bridge_ocl_field,
    },
];

/// BridgeBehaviorModuleData - Configuration for Bridge behavior
#[derive(Clone)]
pub struct BridgeBehaviorModuleData {
    pub base: BehaviorModuleData,
    pub lateral_scaffold_speed: Real,
    pub vertical_scaffold_speed: Real,
    pub fx: Vec<BridgeFXInfo>,
    pub ocl: Vec<BridgeOCLInfo>,
}

impl BridgeBehaviorModuleData {
    pub fn new() -> Self {
        Self {
            base: BehaviorModuleData::new(),
            lateral_scaffold_speed: 1.0,
            vertical_scaffold_speed: 1.0,
            fx: Vec::new(),
            ocl: Vec::new(),
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.fx.clear();
        self.ocl.clear();
        ini.init_from_ini_with_fields(self, BRIDGE_BEHAVIOR_FIELDS)
    }
}

impl Default for BridgeBehaviorModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for BridgeBehaviorModuleData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BridgeBehaviorModuleData")
            .field("lateral_scaffold_speed", &self.lateral_scaffold_speed)
            .field("vertical_scaffold_speed", &self.vertical_scaffold_speed)
            .field("fx_count", &self.fx.len())
            .field("ocl_count", &self.ocl.len())
            .finish()
    }
}

impl ThingModuleData for BridgeBehaviorModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.base.set_module_tag_name_key(key);
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }
}

impl Snapshotable for BridgeBehaviorModuleData {
    fn crc(&self, _xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// BridgeBehavior - Main implementation of bridge behavior
pub struct BridgeBehavior {
    pub module_data: Arc<BridgeBehaviorModuleData>,
    object_id: ObjectID,
    object_handle: Mutex<Option<Weak<RwLock<GameObject>>>>,

    // Tower references
    pub tower_id: [ObjectID; BRIDGE_MAX_TOWERS],

    // Damage and repair effects
    pub damage_to_ocl:
        [[Option<Arc<ObjectCreationList>>; MAX_BRIDGE_BODY_FX]; BODYDAMAGETYPE_COUNT],
    pub damage_to_fx: [[Option<Arc<FXList>>; MAX_BRIDGE_BODY_FX]; BODYDAMAGETYPE_COUNT],
    pub damage_to_sound: [AudioEventRTS; BODYDAMAGETYPE_COUNT],

    pub repair_to_ocl:
        [[Option<Arc<ObjectCreationList>>; MAX_BRIDGE_BODY_FX]; BODYDAMAGETYPE_COUNT],
    pub repair_to_fx: [[Option<Arc<FXList>>; MAX_BRIDGE_BODY_FX]; BODYDAMAGETYPE_COUNT],
    pub repair_to_sound: [AudioEventRTS; BODYDAMAGETYPE_COUNT],

    // State
    pub fx_resolved: Bool,
    pub scaffold_present: Bool,
    pub scaffold_object_id_list: Vec<ObjectID>,
    pub death_frame: UnsignedInt,
    module_fx_handles: Vec<Option<Arc<FXList>>>,
    module_ocl_handles: Vec<Option<Arc<ObjectCreationList>>>,
}

enum ModuleEffectSpawn {
    Position(Coord3D),
    ParentObject,
}

impl BridgeBehavior {
    fn construct_with_object_id(
        object_id: ObjectID,
        module_data: Arc<BridgeBehaviorModuleData>,
        initial_object: Option<Arc<RwLock<GameObject>>>,
    ) -> Self {
        let fx_count = module_data.fx.len();
        let ocl_count = module_data.ocl.len();

        let initial_handle = initial_object
            .as_ref()
            .map(|arc| Arc::downgrade(arc))
            .or_else(|| {
                if object_id == OBJECT_INVALID_ID {
                    None
                } else {
                    OBJECT_REGISTRY
                        .get_object(object_id)
                        .map(|arc| Arc::downgrade(&arc))
                }
            });

        let damage_to_ocl: [[Option<Arc<ObjectCreationList>>; MAX_BRIDGE_BODY_FX];
            BODYDAMAGETYPE_COUNT] = array::from_fn(|_| array::from_fn(|_| None));
        let damage_to_fx: [[Option<Arc<FXList>>; MAX_BRIDGE_BODY_FX]; BODYDAMAGETYPE_COUNT] =
            array::from_fn(|_| array::from_fn(|_| None));
        let repair_to_ocl: [[Option<Arc<ObjectCreationList>>; MAX_BRIDGE_BODY_FX];
            BODYDAMAGETYPE_COUNT] = array::from_fn(|_| array::from_fn(|_| None));
        let repair_to_fx: [[Option<Arc<FXList>>; MAX_BRIDGE_BODY_FX]; BODYDAMAGETYPE_COUNT] =
            array::from_fn(|_| array::from_fn(|_| None));
        let damage_to_sound: [AudioEventRTS; BODYDAMAGETYPE_COUNT] =
            array::from_fn(|_| AudioEventRTS::new());
        let repair_to_sound: [AudioEventRTS; BODYDAMAGETYPE_COUNT] =
            array::from_fn(|_| AudioEventRTS::new());

        Self {
            module_data,
            object_id,
            object_handle: Mutex::new(initial_handle),
            tower_id: [OBJECT_INVALID_ID; BRIDGE_MAX_TOWERS],
            damage_to_ocl,
            damage_to_fx,
            damage_to_sound,
            repair_to_ocl,
            repair_to_fx,
            repair_to_sound,
            fx_resolved: false,
            scaffold_present: false,
            scaffold_object_id_list: Vec::new(),
            death_frame: 0,
            module_fx_handles: vec![None; fx_count],
            module_ocl_handles: vec![None; ocl_count],
        }
    }

    pub fn new_from_object_handle(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<BridgeBehaviorModuleData>,
    ) -> Self {
        let object_id = object
            .read()
            .map(|guard| guard.get_id())
            .unwrap_or(OBJECT_INVALID_ID);

        Self::construct_with_object_id(object_id, module_data, Some(object))
    }

    pub fn from_module_thing(
        thing: Arc<dyn ModuleThing>,
        module_data: Arc<BridgeBehaviorModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let module_object = thing
            .as_object()
            .ok_or_else(|| "BridgeBehavior requires an owning object".to_string())?;

        let object_id = module_object.get_object_id();
        let object = OBJECT_REGISTRY.get_object(object_id).ok_or_else(|| {
            format!("BridgeBehavior requires object {object_id} to be registered")
        })?;

        Ok(Self::new_from_object_handle(object, module_data))
    }

    /// Get interface mask for module registration
    pub fn get_interface_mask() -> Int {
        (crate::modules::MODULEINTERFACE_DAMAGE
            | crate::modules::MODULEINTERFACE_DIE
            | crate::modules::MODULEINTERFACE_UPDATE) as Int
    }

    /// Set a tower for this bridge
    pub fn set_tower(
        &mut self,
        tower_type: BridgeTowerType,
        tower: Option<Arc<RwLock<GameObject>>>,
    ) {
        let index = tower_type as usize;
        if index >= BRIDGE_MAX_TOWERS {
            warn!("BridgeBehavior::set_tower received invalid index {index}");
            return;
        }

        let previous_id = self.tower_id[index];
        let new_id = tower
            .as_ref()
            .and_then(|arc| arc.read().ok().map(|guard| guard.get_id()))
            .unwrap_or(OBJECT_INVALID_ID);

        if previous_id != OBJECT_INVALID_ID && previous_id != new_id {
            if let Err(err) = self.detach_tower(previous_id) {
                warn!("BridgeBehavior failed to detach tower {previous_id}: {err}");
            }
        }

        self.tower_id[index] = new_id;

        if let Some(tower_object) = tower {
            if let Err(err) = self.attach_tower(&tower_object, tower_type) {
                warn!("BridgeBehavior failed to attach tower {new_id}: {err}");
            }
        }
    }

    fn attach_tower(
        &self,
        tower_object: &Arc<RwLock<GameObject>>,
        tower_type: BridgeTowerType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let bridge_arc = self.get_object()?;
        let module_handles = {
            let tower_read = tower_object
                .read()
                .map_err(|e| format!("tower lock poisoned: {}", e))?;
            tower_read.behavior_modules()
        };

        for handle in module_handles {
            let _ =
                handle.with_module_downcast::<BridgeTowerBehaviorModule, _, _>(|tower_module| {
                    let behavior = tower_module.behavior_mut();
                    BridgeTowerBehaviorInterface::set_tower_type(behavior, tower_type);
                    BridgeTowerBehaviorInterface::set_bridge(behavior, Some(bridge_arc.clone()));
                });
        }

        Ok(())
    }

    fn detach_tower(
        &self,
        tower_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if tower_id == OBJECT_INVALID_ID {
            return Ok(());
        }

        if let Some(tower_object) = OBJECT_REGISTRY.get_object(tower_id) {
            let module_handles = {
                let tower_read = tower_object
                    .read()
                    .map_err(|e| format!("tower lock poisoned: {}", e))?;
                tower_read.behavior_modules()
            };

            for handle in module_handles {
                let _ = handle.with_module_downcast::<BridgeTowerBehaviorModule, _, _>(
                    |tower_module| {
                        let behavior = tower_module.behavior_mut();
                        BridgeTowerBehaviorInterface::set_bridge(behavior, None);
                    },
                );
            }
        }

        Ok(())
    }

    /// Get tower ID by type
    pub fn get_tower_id(&self, tower_type: BridgeTowerType) -> ObjectID {
        let index = tower_type as usize;
        if index < BRIDGE_MAX_TOWERS {
            self.tower_id[index]
        } else {
            OBJECT_INVALID_ID
        }
    }

    /// Create scaffolding around bridge
    pub fn create_scaffolding(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.scaffold_present {
            return Ok(());
        }

        let me = self.get_object()?;
        let (position, team) = {
            let me_read = me
                .read()
                .map_err(|e| format!("bridge object lock poisoned: {}", e))?;
            (*me_read.get_position(), me_read.get_team())
        };

        let Some(team_arc) = team else {
            return Ok(());
        };

        let Some(bridge) = self.find_bridge_at_position(&position)? else {
            return Ok(());
        };

        let Some(template) = self.get_bridge_template(&bridge)? else {
            return Ok(());
        };

        self.create_scaffold_objects(&template, &bridge, &team_arc)?;
        self.scaffold_present = true;
        self.update_bridge_pathfinding(&bridge, false);

        Ok(())
    }

    /// Remove scaffolding around bridge
    pub fn remove_scaffolding(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.scaffold_present {
            return Ok(());
        }

        // Reverse the motion on all scaffold objects.
        for &scaffold_id in &self.scaffold_object_id_list {
            if let Some(scaffold_obj) = self.find_object_by_id(scaffold_id)? {
                let _ = self.with_scaffold_interface(&scaffold_obj, |interface| {
                    interface.reverse_motion();
                });
            }
        }

        self.scaffold_object_id_list.clear();
        self.scaffold_present = false;

        let allow_passable = {
            let me = self.get_object()?;
            let me_read = me
                .read()
                .map_err(|e| format!("bridge object lock poisoned: {}", e))?;
            match me_read.get_body_module() {
                Some(body) => match body.lock() {
                    Ok(body_guard) => body_guard.get_damage_state() != BodyDamageType::Rubble,
                    Err(_) => false,
                },
                None => false,
            }
        };
        if allow_passable {
            let me = self.get_object()?;
            let me_read = me
                .read()
                .map_err(|e| format!("bridge object lock poisoned: {}", e))?;
            if let Some(bridge) = self.find_bridge_at_position(me_read.get_position())? {
                self.update_bridge_pathfinding(&bridge, true);
            }
        }

        Ok(())
    }

    /// Check if scaffold is in motion
    pub fn is_scaffold_in_motion(&self) -> Bool {
        for &scaffold_id in &self.scaffold_object_id_list {
            if let Ok(Some(scaffold_obj)) = self.find_object_by_id(scaffold_id) {
                if let Some(motion) = self.with_scaffold_interface(&scaffold_obj, |interface| {
                    interface.get_current_motion()
                }) {
                    if motion != ScaffoldTargetMotion::Still {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Check if scaffold is present
    pub fn is_scaffold_present(&self) -> Bool {
        self.scaffold_present
    }

    fn update_bridge_pathfinding(&self, bridge: &Bridge, passable: bool) {
        let bridge_info = bridge.get_bridge_info();
        let polygon = [
            bridge_info.from_left,
            bridge_info.from_right,
            bridge_info.to_right,
            bridge_info.to_left,
        ];

        if let Ok(ai_guard) = THE_AI.read() {
            if let Some(pathfinding) = ai_guard.pathfinding_system() {
                if let Ok(mut pathfinding_guard) = pathfinding.write() {
                    let layer =
                        AiPathfindLayerEnum::from(terrain_layer_to_logic_layer(bridge.get_layer()));
                    pathfinding_guard.set_bridge_passable(&polygon, layer, passable);
                }
            }
        }
    }

    /// Get bridge behavior interface from object
    pub fn get_bridge_behavior_interface_from_object(
        obj: Arc<RwLock<GameObject>>,
    ) -> Option<Arc<Mutex<dyn BridgeBehaviorInterface>>> {
        let _ = obj;
        None
    }

    /// Handle deletion cleanup
    pub fn on_delete(&mut self) {
        self.scaffold_object_id_list.clear();
    }

    /// Resolve FX references
    fn resolve_fx(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.fx_resolved {
            return Ok(());
        }

        self.resolve_module_resources();

        let me = self.get_object()?;
        let me_read = me
            .read()
            .map_err(|e| format!("bridge object lock poisoned: {}", e))?;
        let position = me_read.get_position();

        // Find bridge at our position
        if let Some(bridge) = self.find_bridge_at_position(&position)? {
            if let Some(bridge_template) = self.get_bridge_template(&bridge)? {
                self.resolve_bridge_fx(&bridge_template)?;
            }
        }

        self.fx_resolved = true;
        Ok(())
    }

    fn play_audio_event(&self, _event: &AudioEventRTS) {}

    fn get_object_position(
        &self,
        object: &Arc<RwLock<GameObject>>,
    ) -> Result<Coord3D, Box<dyn std::error::Error + Send + Sync>> {
        let guard = object
            .read()
            .map_err(|e| format!("bridge object lock poisoned: {}", e))?;
        Ok(*guard.get_position())
    }

    fn get_bone_position(
        &self,
        object: &Arc<RwLock<GameObject>>,
        bone_name: &str,
    ) -> Result<Option<Coord3D>, Box<dyn std::error::Error + Send + Sync>> {
        let drawable = {
            let guard = object
                .read()
                .map_err(|e| format!("bridge object lock poisoned: {}", e))?;
            guard.get_drawable()
        };

        if let Some(drawable) = drawable {
            if let Ok(drawable_guard) = drawable.read() {
                if let Some(transform) = drawable_guard.get_bone_transform(bone_name) {
                    let cols = transform.to_cols_array();
                    return Ok(Some(Coord3D::new(cols[12], cols[13], cols[14])));
                }
            }
        }

        Ok(None)
    }

    fn resolve_spawn_target(
        &self,
        object: &Arc<RwLock<GameObject>>,
        template: Option<&TerrainRoadType>,
        bridge: Option<&Bridge>,
        info: &TimeAndLocationInfo,
        fallback: Coord3D,
    ) -> Result<ModuleEffectSpawn, Box<dyn std::error::Error + Send + Sync>> {
        let bone_name = info.bone_name.as_str().trim();
        if !bone_name.is_empty() {
            if bone_name.eq_ignore_ascii_case("ParentObject") {
                return Ok(ModuleEffectSpawn::ParentObject);
            }

            if let Some(position) = self.get_bone_position(object, bone_name)? {
                return Ok(ModuleEffectSpawn::Position(position));
            }

            return Ok(ModuleEffectSpawn::Position(fallback));
        }

        if let (Some(template_ref), Some(bridge_ref)) = (template, bridge) {
            let position = self.get_random_surface_position(template_ref, bridge_ref)?;
            return Ok(ModuleEffectSpawn::Position(position));
        }

        Ok(ModuleEffectSpawn::Position(fallback))
    }

    fn execute_ocl_on_object(
        &self,
        ocl: &ObjectCreationList,
        _object: &Arc<RwLock<GameObject>>,
        position: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        ocl.create_at_position(position, self.object_id)
    }

    /// Resolve bridge-specific FX
    fn resolve_bridge_fx(
        &mut self,
        bridge_template: &TerrainRoadType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for state_index in 0..BODYDAMAGETYPE_COUNT {
            for effect_index in 0..MAX_BRIDGE_BODY_FX {
                // Damage FX
                if let Some(name) =
                    bridge_template.get_damage_to_fx_string(state_index, effect_index)
                {
                    let trimmed = name.as_str().trim();
                    if !trimmed.is_empty() {
                        if let Some(handle) = TheFXListStore::find_fx_list(trimmed) {
                            self.damage_to_fx[state_index][effect_index] = Some(handle);
                        } else {
                            log::debug!(
                                "BridgeBehavior: FX '{}' not registered; skipping unresolved entry",
                                trimmed
                            );
                        }
                    }
                }

                // Damage OCL
                if let Some(name) =
                    bridge_template.get_damage_to_ocl_string(state_index, effect_index)
                {
                    let trimmed = name.as_str().trim();
                    if !trimmed.is_empty() {
                        if let Some(handle) =
                            TheObjectCreationListStore::find_object_creation_list(trimmed)
                        {
                            self.damage_to_ocl[state_index][effect_index] = Some(handle);
                        } else {
                            log::debug!(
                                "BridgeBehavior: OCL '{}' not registered; skipping unresolved entry",
                                trimmed
                            );
                        }
                    }
                }

                // Repair FX
                if let Some(name) =
                    bridge_template.get_repaired_to_fx_string(state_index, effect_index)
                {
                    let trimmed = name.as_str().trim();
                    if !trimmed.is_empty() {
                        if let Some(handle) = TheFXListStore::find_fx_list(trimmed) {
                            self.repair_to_fx[state_index][effect_index] = Some(handle);
                        } else {
                            log::debug!(
                                "BridgeBehavior: repair FX '{}' not registered; skipping unresolved entry",
                                trimmed
                            );
                        }
                    }
                }

                // Repair OCL
                if let Some(name) =
                    bridge_template.get_repaired_to_ocl_string(state_index, effect_index)
                {
                    let trimmed = name.as_str().trim();
                    if !trimmed.is_empty() {
                        if let Some(handle) =
                            TheObjectCreationListStore::find_object_creation_list(trimmed)
                        {
                            self.repair_to_ocl[state_index][effect_index] = Some(handle);
                        } else {
                            log::debug!(
                                "BridgeBehavior: repair OCL '{}' not registered; skipping unresolved entry",
                                trimmed
                            );
                        }
                    }
                }
            }

            // Damage sounds
            if let Some(name) = bridge_template.get_damage_to_sound_string(state_index) {
                let trimmed = name.as_str().trim();
                self.damage_to_sound[state_index].sound_file = trimmed.to_string();
            } else {
                self.damage_to_sound[state_index].sound_file.clear();
            }

            // Repair sounds
            if let Some(name) = bridge_template.get_repaired_to_sound_string(state_index) {
                let trimmed = name.as_str().trim();
                self.repair_to_sound[state_index].sound_file = trimmed.to_string();
            } else {
                self.repair_to_sound[state_index].sound_file.clear();
            }
        }

        // Ensure at least the final damage state has FX/OCL entries by falling back to
        // the module-local resources if the bridge template did not specify any.

        Ok(())
    }

    /// Resolve FX/OCL handles defined on the module data
    fn resolve_module_resources(&mut self) {
        if self.module_fx_handles.len() < self.module_data.fx.len() {
            self.module_fx_handles
                .resize(self.module_data.fx.len(), None);
        }
        if self.module_ocl_handles.len() < self.module_data.ocl.len() {
            self.module_ocl_handles
                .resize(self.module_data.ocl.len(), None);
        }

        for (index, fx_info) in self.module_data.fx.iter().enumerate() {
            if self.module_fx_handles[index].is_some() {
                continue;
            }

            if let Some(name) = &fx_info.fx_name {
                if let Some(handle) = TheFXListStore::find_fx_list(name.as_str()) {
                    self.module_fx_handles[index] = Some(handle);
                } else {
                    log::debug!(
                        "BridgeBehavior: FX '{}' not registered; skipping unresolved entry",
                        name
                    );
                }
            }
        }

        for (index, ocl_info) in self.module_data.ocl.iter().enumerate() {
            if self.module_ocl_handles[index].is_some() {
                continue;
            }

            if let Some(name) = &ocl_info.ocl_name {
                if let Some(handle) =
                    TheObjectCreationListStore::find_object_creation_list(name.as_str())
                {
                    self.module_ocl_handles[index] = Some(handle);
                } else {
                    log::debug!(
                        "BridgeBehavior: OCL '{}' not registered; skipping unresolved entry",
                        name
                    );
                }
            }
        }
    }

    /// Handle objects on bridge when bridge dies
    fn handle_objects_on_bridge_on_die(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let me = self.get_object()?;
        let me_read = me
            .read()
            .map_err(|e| format!("bridge object lock poisoned: {}", e))?;
        let bridge_pos = *me_read.get_position();
        drop(me_read);

        let Some(bridge) = self.find_bridge_at_position(&bridge_pos)? else {
            return Ok(());
        };

        let bridge_layer = terrain_layer_to_logic_layer(bridge.get_layer());
        let bridge_info = bridge.get_bridge_info();
        let bridge_polygon = [
            bridge_info.from_left,
            bridge_info.from_right,
            bridge_info.to_right,
            bridge_info.to_left,
        ];

        let mut low_bridge_z = bridge_polygon[0].z;
        for corner in &bridge_polygon {
            if corner.z < low_bridge_z {
                low_bridge_z = corner.z;
            }
        }

        let dx = bridge_info.to_left.x - bridge_pos.x;
        let dy = bridge_info.to_left.y - bridge_pos.y;
        let radius = (dx * dx + dy * dy).sqrt();

        let Some(partition) = ThePartitionManager::get() else {
            return Ok(());
        };

        for object_id in partition.get_objects_in_range(&bridge_pos, radius) {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
                continue;
            };

            let (pos, layer, is_bridge, is_tower, above) = {
                let obj_read = match obj_arc.read() {
                    Ok(guard) => guard,
                    Err(_) => continue,
                };
                (
                    *obj_read.get_position(),
                    obj_read.get_layer(),
                    obj_read.is_kind_of(KindOf::Bridge),
                    obj_read.is_kind_of(KindOf::BridgeTower),
                    obj_read.is_above_terrain(),
                )
            };

            if is_bridge || is_tower {
                continue;
            }
            if above {
                continue;
            }
            if pos.z < low_bridge_z {
                continue;
            }
            if !point_inside_area_2d(&pos, &bridge_polygon) {
                continue;
            }
            if layer != bridge_layer {
                continue;
            }

            let mut obj_write = match obj_arc.write() {
                Ok(guard) => guard,
                Err(_) => continue,
            };
            if obj_write.get_layer() == bridge_layer {
                obj_write.set_layer(PathfindLayerEnum::Ground);
            }

            if let Some(physics) = obj_write.get_physics() {
                physics.set_allow_to_fall(true);
            } else {
                obj_write.kill(None, None);
            }
        }

        Ok(())
    }

    /// Perform area effects when bridge changes state
    fn do_area_effects(
        &self,
        bridge_template: &TerrainRoadType,
        bridge: &Bridge,
        ocl: Option<&ObjectCreationList>,
        fx: Option<&FXList>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if ocl.is_none() && fx.is_none() {
            return Ok(());
        }

        let iterations = bridge_template.num_fx_per_type;
        if iterations == 0 {
            return Ok(());
        }

        for _ in 0..iterations {
            if let Some(fx_ref) = fx {
                let position = self.get_random_surface_position(bridge_template, bridge)?;
                self.execute_fx_at_position(fx_ref, &position)?;
            }

            if let Some(ocl_ref) = ocl {
                let position = self.get_random_surface_position(bridge_template, bridge)?;
                self.execute_ocl_at_position(ocl_ref, &position)?;
            }
        }

        Ok(())
    }

    fn with_scaffold_interface<R>(
        &self,
        obj: &Arc<RwLock<GameObject>>,
        f: impl FnOnce(&mut dyn BridgeScaffoldBehaviorInterface) -> R,
    ) -> Option<R> {
        let behaviors = obj.read().ok()?.get_behavior_modules();
        for behavior in behaviors {
            if let Ok(mut guard) = behavior.lock() {
                if let Some(interface) = guard.get_bridge_scaffold_behavior_interface() {
                    return Some(f(interface));
                }
            }
        }
        None
    }

    /// Set scaffold data for positioning
    fn set_scaffold_data(
        &self,
        obj: Arc<RwLock<GameObject>>,
        angle: Real,
        sunken_height: Real,
        rise_to_pos: &Coord3D,
        build_pos: &Coord3D,
        bridge_center: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut sunken_pos = *rise_to_pos;
        let fudge = 8.0;
        sunken_pos.z -= sunken_height.max(0.0) + fudge;

        {
            let mut obj_write = obj
                .write()
                .map_err(|e| format!("bridge object write lock poisoned: {}", e))?;
            obj_write.set_position(&sunken_pos)?;
            obj_write.set_orientation(angle)?;
        }

        let build_to_center = *build_pos - *rise_to_pos;
        let rise_to_center = *bridge_center - *rise_to_pos;
        let dist_build_to_center = build_to_center.length();
        let dist_rise_to_center = rise_to_center.length();
        let ratio = if dist_rise_to_center > f32::EPSILON {
            dist_build_to_center / dist_rise_to_center
        } else {
            1.0
        };
        let lateral_speed = self.module_data.lateral_scaffold_speed * ratio;
        let vertical_speed = self.module_data.vertical_scaffold_speed;

        let applied = self.with_scaffold_interface(&obj, |scaffold_behavior| {
            scaffold_behavior.set_positions(&sunken_pos, rise_to_pos, build_pos);
            scaffold_behavior.set_motion(ScaffoldTargetMotion::Rise);
            scaffold_behavior.set_lateral_speed(lateral_speed);
            scaffold_behavior.set_vertical_speed(vertical_speed);
        });

        if applied.is_none() {
            return Err(
                "BridgeBehavior::set_scaffold_data missing BridgeScaffoldBehavior interface".into(),
            );
        }

        Ok(())
    }

    /// Get random surface position on bridge
    fn get_random_surface_position(
        &self,
        bridge_template: &TerrainRoadType,
        bridge: &Bridge,
    ) -> Result<Coord3D, Box<dyn std::error::Error + Send + Sync>> {
        let info = bridge.get_bridge_info();

        let mut v1 = info.to_left - info.from_left;
        let scale1 = get_game_logic_random_value_real(0.0, 1.0);
        v1 *= scale1;

        let mut v2 = info.from_right - info.from_left;
        let scale2 = get_game_logic_random_value_real(0.0, 1.0);
        v2 *= scale2;

        let mut position = info.from_left + v1 + v2;

        let height_range = bridge_template.transition_effects_height.max(0.0);
        if height_range > 0.0 {
            position.z += get_game_logic_random_value_real(0.0, height_range);
        }

        Ok(position)
    }

    /// Create scaffold objects along the bridge
    fn create_scaffold_objects(
        &mut self,
        bridge_template: &TerrainRoadType,
        bridge: &Bridge,
        team: &Arc<RwLock<Team>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.scaffold_object_id_list.clear();

        let scaffold_name = bridge_template.scaffold_object_name.as_str().trim();
        if scaffold_name.is_empty() {
            return Ok(());
        }
        let support_name = bridge_template.scaffold_support_object_name.as_str().trim();
        if support_name.is_empty() {
            return Ok(());
        }

        let scaffold_template = TheThingFactory::find_template(scaffold_name).ok_or_else(|| {
            format!(
                "BridgeBehavior::create_scaffold_objects missing scaffold template '{}'",
                scaffold_name
            )
        })?;
        let support_template = TheThingFactory::find_template(support_name).ok_or_else(|| {
            format!(
                "BridgeBehavior::create_scaffold_objects missing scaffold support template '{}'",
                support_name
            )
        })?;

        let scaffold_geometry = scaffold_template.get_template_geometry_info();
        let support_geometry = support_template.get_template_geometry_info();

        let spacing = scaffold_geometry.get_major_radius() * 2.0;
        if spacing <= 0.0 {
            return Ok(());
        }

        let scaffold_height = scaffold_geometry.get_max_height_above_position()
            + scaffold_geometry.get_max_height_below_position();
        let support_height = support_geometry.get_max_height_above_position()
            + support_geometry.get_max_height_below_position();

        let bridge_info = bridge.get_bridge_info();
        let center = {
            let me = self.get_object()?;
            let me_read = me
                .read()
                .map_err(|e| format!("bridge object lock poisoned: {}", e))?;
            *me_read.get_position()
        };

        let left_start = (bridge_info.from_left + bridge_info.from_right) * 0.5;
        let right_start = (bridge_info.to_left + bridge_info.to_right) * 0.5;

        let angle_vec = right_start - left_start;
        let left_angle = angle_vec.y.atan2(angle_vec.x);
        let right_angle = left_angle + TAU;

        let mut left_vector = right_start - left_start;
        let mut right_vector = left_start - right_start;

        let tile_distance = left_vector.length();
        if tile_distance <= 0.0 {
            return Ok(());
        }

        let num_objects = (tile_distance / spacing).ceil() as usize + 1;
        let num_iterations = ((num_objects as f32) / 2.0).ceil() as usize;

        left_vector = left_vector.normalize();
        right_vector = right_vector.normalize();

        let factory = TheThingFactory::get()?;
        let team_guard = team
            .read()
            .map_err(|_| "BridgeBehavior::create_scaffold_objects team lock poisoned")?;

        let mut scaffold_objects_created = 0usize;
        for i in 0..num_iterations {
            if scaffold_objects_created >= num_objects {
                break;
            }

            let spacing_offset = spacing * (i as f32);

            // Left side scaffold
            let rise_to_pos = left_start;
            let mut destination_pos =
                left_vector * spacing_offset + rise_to_pos + Coord3D::new(0.1, 0.0, 0.0);
            destination_pos.z = left_vector.z * spacing_offset + rise_to_pos.z;

            let obj = factory.new_object(scaffold_template.clone(), &team_guard)?;
            let obj_id = obj
                .read()
                .map(|guard| guard.get_id())
                .unwrap_or(OBJECT_INVALID_ID);
            self.set_scaffold_data(
                obj,
                left_angle,
                scaffold_height,
                &rise_to_pos,
                &destination_pos,
                &center,
            )?;
            if obj_id != OBJECT_INVALID_ID {
                self.scaffold_object_id_list.push(obj_id);
            }
            scaffold_objects_created += 1;

            // Support scaffolds under left side
            if support_height > 0.0 {
                let mut offset = rise_to_pos.z;
                let mut support_rise = rise_to_pos;
                let mut support_destination = destination_pos;
                let mut support_center = center;
                while offset >= 0.0 {
                    support_rise.z -= support_height;
                    support_destination.z -= support_height;
                    support_center.z -= support_height;
                    let support_obj = factory.new_object(support_template.clone(), &team_guard)?;
                    let support_id = support_obj
                        .read()
                        .map(|guard| guard.get_id())
                        .unwrap_or(OBJECT_INVALID_ID);
                    self.set_scaffold_data(
                        support_obj,
                        left_angle,
                        support_height,
                        &support_rise,
                        &support_destination,
                        &support_center,
                    )?;
                    if support_id != OBJECT_INVALID_ID {
                        self.scaffold_object_id_list.push(support_id);
                    }
                    offset -= support_height;
                }
            }

            if scaffold_objects_created >= num_objects {
                continue;
            }

            // Right side scaffold
            let rise_to_pos = right_start;
            let mut destination_pos =
                right_vector * spacing_offset + rise_to_pos + Coord3D::new(0.1, 0.0, 0.0);
            destination_pos.z = right_vector.z * spacing_offset + rise_to_pos.z;

            let obj = factory.new_object(scaffold_template.clone(), &team_guard)?;
            let obj_id = obj
                .read()
                .map(|guard| guard.get_id())
                .unwrap_or(OBJECT_INVALID_ID);
            self.set_scaffold_data(
                obj,
                right_angle,
                scaffold_height,
                &rise_to_pos,
                &destination_pos,
                &center,
            )?;
            if obj_id != OBJECT_INVALID_ID {
                self.scaffold_object_id_list.push(obj_id);
            }
            scaffold_objects_created += 1;

            // Support scaffolds under right side
            if support_height > 0.0 {
                let mut offset = rise_to_pos.z;
                let mut support_rise = rise_to_pos;
                let mut support_destination = destination_pos;
                let mut support_center = center;
                while offset >= 0.0 {
                    support_rise.z -= support_height;
                    support_destination.z -= support_height;
                    support_center.z -= support_height;
                    let support_obj = factory.new_object(support_template.clone(), &team_guard)?;
                    let support_id = support_obj
                        .read()
                        .map(|guard| guard.get_id())
                        .unwrap_or(OBJECT_INVALID_ID);
                    self.set_scaffold_data(
                        support_obj,
                        right_angle,
                        support_height,
                        &support_rise,
                        &support_destination,
                        &support_center,
                    )?;
                    if support_id != OBJECT_INVALID_ID {
                        self.scaffold_object_id_list.push(support_id);
                    }
                    offset -= support_height;
                }
            }
        }

        Ok(())
    }

    /// Find bridge at position
    fn find_bridge_at_position(
        &self,
        position: &Coord3D,
    ) -> Result<Option<Bridge>, Box<dyn std::error::Error + Send + Sync>> {
        let terrain = THE_TERRAIN_LOGIC
            .read()
            .map_err(|_| "Failed to lock terrain logic".to_string())?;
        Ok(terrain.find_bridge_at(position).cloned())
    }

    /// Get bridge template
    fn get_bridge_template(
        &self,
        bridge: &Bridge,
    ) -> Result<Option<TerrainRoadType>, Box<dyn std::error::Error + Send + Sync>> {
        let name = EngineAsciiString::from(bridge.get_bridge_template_name().as_str());
        Ok(IniTerrainBridge::find_terrain_bridge_by_name(&name))
    }

    /// Execute OCL at position
    fn execute_ocl_at_position(
        &self,
        ocl: &ObjectCreationList,
        position: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        ocl.create_at_position(position, OBJECT_INVALID_ID)
    }

    /// Execute FX at position
    fn execute_fx_at_position(
        &self,
        fx: &FXList,
        position: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        fx.do_fx_at_position(position)
    }

    /// Find object by ID
    fn find_object_by_id(
        &self,
        id: ObjectID,
    ) -> Result<Option<Arc<RwLock<GameObject>>>, Box<dyn std::error::Error + Send + Sync>> {
        if id == OBJECT_INVALID_ID {
            return Ok(None);
        }

        Ok(OBJECT_REGISTRY.get_object(id))
    }

    /// Get the object this behavior belongs to
    fn get_object(
        &self,
    ) -> Result<Arc<RwLock<GameObject>>, Box<dyn std::error::Error + Send + Sync>> {
        if self.object_id == OBJECT_INVALID_ID {
            return Err("BridgeBehavior missing owning object id".into());
        }

        if let Ok(mut handle) = self.object_handle.lock() {
            if let Some(weak) = handle.as_ref() {
                if let Some(object) = weak.upgrade() {
                    return Ok(object);
                }
            }

            if let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) {
                *handle = Some(Arc::downgrade(&object));
                return Ok(object);
            }
        }

        Err(format!(
            "BridgeBehavior unable to upgrade handle for object {}",
            self.object_id
        )
        .into())
    }

    /// Get current game frame
    fn get_current_frame(&self) -> Result<UnsignedInt, Box<dyn std::error::Error + Send + Sync>> {
        Ok(TheGameLogic::get_frame())
    }

    pub fn crc(
        &self,
        _xfer: &mut dyn EngineXfer,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    pub fn xfer(
        &mut self,
        xfer: &mut dyn EngineXfer,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        if xfer.get_xfer_mode() == XferMode::Load {
            if let Ok(mut terrain) = THE_TERRAIN_LOGIC.write() {
                if let Ok(me) = self.get_object() {
                    if let Ok(me_read) = me.read() {
                        if let Some(bridge) = terrain.find_bridge_at_mut(me_read.get_position()) {
                            bridge.set_bridge_object_id(me_read.get_id());
                        }
                    }
                }
            }
        }

        for tower_id in &mut self.tower_id {
            xfer.xfer_object_id(tower_id)?;
        }

        if xfer.get_xfer_mode() == XferMode::Load {
            if let Ok(mut terrain) = THE_TERRAIN_LOGIC.write() {
                if let Ok(me) = self.get_object() {
                    if let Ok(me_read) = me.read() {
                        if let Some(bridge) = terrain.find_bridge_at_mut(me_read.get_position()) {
                            for (index, tower_id) in self.tower_id.iter().copied().enumerate() {
                                if let Some(tower_type) =
                                    terrain_bridge_tower_type_from_index(index)
                                {
                                    bridge.set_tower_object_id(tower_id, tower_type);
                                }
                            }
                        }
                    }
                }
            }
        }

        xfer.xfer_bool(&mut self.scaffold_present)?;

        let mut scaffold_count: u16 =
            self.scaffold_object_id_list.len().min(u16::MAX as usize) as u16;
        xfer.xfer_u16(&mut scaffold_count);
        if xfer.get_xfer_mode() == XferMode::Save {
            for object_id in self
                .scaffold_object_id_list
                .iter()
                .take(scaffold_count as usize)
            {
                let mut id_copy = *object_id;
                xfer.xfer_object_id(&mut id_copy)?;
            }
        } else {
            self.scaffold_object_id_list.clear();
            for _ in 0..scaffold_count {
                let mut object_id = OBJECT_INVALID_ID;
                xfer.xfer_object_id(&mut object_id)?;
                self.scaffold_object_id_list.push(object_id);
            }
        }

        xfer.xfer_unsigned_int(&mut self.death_frame)?;

        Ok(())
    }

    pub fn load_post_process(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

// Implement DamageModuleInterface
impl DamageModuleInterface for BridgeBehavior {
    fn receive_damage(&mut self, _object_id: ObjectID, _damage: &DamageInfo) -> Real {
        0.0
    }

    fn on_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.resolve_fx()?;

        let me = self.get_object()?;
        let me_read = me
            .read()
            .map_err(|e| format!("bridge object lock poisoned: {}", e))?;
        let me_id = me_read.get_id();
        let body = match me_read.get_body_module() {
            Some(body) => body,
            None => return Ok(()),
        };

        let max_health = body
            .lock()
            .map_err(|_| "BridgeBehavior::on_damage body lock poisoned")?
            .get_max_health();
        if max_health <= 0.0 {
            return Ok(());
        }

        let damage_percentage = damage_info.amount / max_health;
        let source_id = damage_info.source_id;
        drop(me_read);

        let source_is_bridge_tower = OBJECT_REGISTRY
            .get_object(source_id)
            .and_then(|source| {
                source
                    .read()
                    .ok()
                    .map(|guard| guard.is_kind_of(KindOf::BridgeTower))
            })
            .unwrap_or(false);

        if !source_is_bridge_tower {
            for tower_id in &self.tower_id {
                let Some(tower_arc) = self.find_object_by_id(*tower_id)? else {
                    continue;
                };

                let tower_max = {
                    let tower_read = match tower_arc.read() {
                        Ok(guard) => guard,
                        Err(_) => continue,
                    };
                    let Some(tower_body) = tower_read.get_body_module() else {
                        continue;
                    };
                    let tower_health = tower_body
                        .lock()
                        .map_err(|_| "BridgeBehavior::on_damage tower body lock poisoned")?
                        .get_max_health();
                    tower_health
                };

                if tower_max <= 0.0 {
                    continue;
                }

                let mut tower_damage = DamageInfo::with_simple(
                    damage_percentage * tower_max,
                    me_id,
                    damage_info.damage_type,
                    damage_info.death_type,
                );
                {
                    let Ok(mut tower_write) = tower_arc.write() else {
                        continue;
                    };
                    let _ = tower_write.attempt_damage(&mut tower_damage);
                }
            }
        }

        Ok(())
    }

    fn on_healing(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.resolve_fx()?;

        let me = self.get_object()?;
        let me_read = me
            .read()
            .map_err(|e| format!("bridge object lock poisoned: {}", e))?;
        let body = match me_read.get_body_module() {
            Some(body) => body,
            None => return Ok(()),
        };

        let max_health = body
            .lock()
            .map_err(|_| "BridgeBehavior::on_healing body lock poisoned")?
            .get_max_health();
        if max_health <= 0.0 {
            return Ok(());
        }

        let healing_percentage = damage_info.amount / max_health;
        let source_id = damage_info.source_id;
        drop(me_read);

        let source_is_bridge_tower = OBJECT_REGISTRY
            .get_object(source_id)
            .and_then(|source| {
                source
                    .read()
                    .ok()
                    .map(|guard| guard.is_kind_of(KindOf::BridgeTower))
            })
            .unwrap_or(false);

        if !source_is_bridge_tower {
            for tower_id in &self.tower_id {
                let Some(tower_arc) = self.find_object_by_id(*tower_id)? else {
                    continue;
                };

                let tower_max = {
                    let tower_read = match tower_arc.read() {
                        Ok(guard) => guard,
                        Err(_) => continue,
                    };
                    let Some(tower_body) = tower_read.get_body_module() else {
                        continue;
                    };
                    let tower_health = tower_body
                        .lock()
                        .map_err(|_| "BridgeBehavior::on_healing tower body lock poisoned")?
                        .get_max_health();
                    tower_health
                };

                if tower_max <= 0.0 {
                    continue;
                }

                {
                    let Ok(mut tower_write) = tower_arc.write() else {
                        continue;
                    };
                    let source_guard = me.read().map_err(|_| "bridge lock poisoned")?;
                    let _ = tower_write
                        .attempt_healing(healing_percentage * tower_max, Some(&*source_guard));
                }
            }
        }

        Ok(())
    }

    fn on_body_damage_state_change(
        &mut self,
        damage_info: &DamageInfo,
        old_state: BodyDamageType,
        new_state: BodyDamageType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Matches C++ BridgeBehavior.cpp:587-678

        // If we're transitioning back from rubble, clear death frame
        if new_state != BodyDamageType::Rubble {
            self.death_frame = 0;
        }

        // Resolve FX if not already done
        if !self.fx_resolved {
            self.resolve_fx()?;
        }

        if !self.fx_resolved {
            return Ok(());
        }

        // States should be different
        if old_state == new_state {
            return Ok(());
        }

        let me = self.get_object()?;
        let me_read = me
            .read()
            .map_err(|e| format!("bridge lock poisoned: {}", e))?;
        let position = *me_read.get_position(); // Copy the Coord3D value
        drop(me_read);

        let bridge = self.find_bridge_at_position(&position)?;
        let bridge_template = if let Some(ref bridge_ref) = bridge {
            self.get_bridge_template(bridge_ref)?
        } else {
            return Err("BridgeBehavior: Unable to find bridge".into());
        };

        if bridge_template.is_none() {
            return Err("BridgeBehavior: Unable to find bridge template".into());
        }

        let template_ref = bridge_template.as_ref().unwrap();
        let bridge_ref = bridge.as_ref().unwrap();

        // Determine if we got repaired (old state was worse than new state) or damaged
        let got_repaired = is_condition_worse(old_state, new_state);

        let new_state_index = damage_type_to_index(new_state);

        if got_repaired {
            // Play repair sound
            self.play_audio_event(&self.repair_to_sound[new_state_index]);

            // Do repair FX and OCL
            for i in 0..MAX_BRIDGE_BODY_FX {
                let fx = self.repair_to_fx[new_state_index][i]
                    .as_ref()
                    .map(|arc| arc.as_ref());
                let ocl = self.repair_to_ocl[new_state_index][i]
                    .as_ref()
                    .map(|arc| arc.as_ref());
                self.do_area_effects(template_ref, bridge_ref, ocl, fx)?;
            }
        } else {
            // Play damage sound
            self.play_audio_event(&self.damage_to_sound[new_state_index]);

            // Do damage FX and OCL
            for i in 0..MAX_BRIDGE_BODY_FX {
                let fx = self.damage_to_fx[new_state_index][i]
                    .as_ref()
                    .map(|arc| arc.as_ref());
                let ocl = self.damage_to_ocl[new_state_index][i]
                    .as_ref()
                    .map(|arc| arc.as_ref());
                self.do_area_effects(template_ref, bridge_ref, ocl, fx)?;
            }
        }

        if new_state == BodyDamageType::Rubble {
            self.update_bridge_pathfinding(bridge_ref, false);
        } else if old_state == BodyDamageType::Rubble && !self.scaffold_present {
            self.update_bridge_pathfinding(bridge_ref, true);
        }

        // Update bridge damage states in terrain logic
        if let Ok(mut terrain) = THE_TERRAIN_LOGIC.write() {
            terrain.update_bridge_damage_states();
        }

        // If transitioning to/from rubble, queue radar terrain refresh
        if old_state == BodyDamageType::Rubble || new_state == BodyDamageType::Rubble {
            if let Some(radar) = TheRadar::get() {
                radar.refresh_terrain();
            }
        }

        Ok(())
    }
}

// Implement DieModuleInterface
impl DieModuleInterface for BridgeBehavior {
    fn on_die(
        &mut self,
        _damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for tower_id in &self.tower_id {
            if let Some(tower_arc) = self.find_object_by_id(*tower_id)? {
                if let Ok(mut tower_write) = tower_arc.write() {
                    tower_write.kill(None, None);
                }
            }
        }

        self.handle_objects_on_bridge_on_die()?;
        self.death_frame = self.get_current_frame()?;

        Ok(())
    }
}

// Implement UpdateModuleInterface
impl UpdateModuleInterface for BridgeBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        if self.death_frame != 0 {
            self.resolve_module_resources();

            let current_frame = self.get_current_frame()?;
            let death_time = current_frame.saturating_sub(self.death_frame);

            let object = self.get_object()?;
            let object_position = self.get_object_position(&object)?;

            let bridge = self.find_bridge_at_position(&object_position)?;
            let bridge_template = if let Some(ref bridge_ref) = bridge {
                self.get_bridge_template(bridge_ref)?
            } else {
                None
            };

            let bridge_ref = bridge.as_ref();
            let template_ref = bridge_template.as_ref();
            for (index, fx_info) in self.module_data.fx.iter().enumerate() {
                let delay = fx_info.time_and_location_info.delay;
                if delay != death_time {
                    continue;
                }

                if let Some(handle_arc) = self
                    .module_fx_handles
                    .get(index)
                    .and_then(|opt| opt.as_ref().cloned())
                {
                    let spawn = self.resolve_spawn_target(
                        &object,
                        template_ref,
                        bridge_ref,
                        &fx_info.time_and_location_info,
                        object_position,
                    )?;
                    match spawn {
                        ModuleEffectSpawn::Position(pos) => {
                            self.execute_fx_at_position(handle_arc.as_ref(), &pos)?;
                        }
                        ModuleEffectSpawn::ParentObject => {
                            self.execute_fx_at_position(handle_arc.as_ref(), &object_position)?;
                        }
                    }
                }
            }

            for (index, ocl_info) in self.module_data.ocl.iter().enumerate() {
                let delay = ocl_info.time_and_location_info.delay;
                if delay != death_time {
                    continue;
                }

                if let Some(handle_arc) = self
                    .module_ocl_handles
                    .get(index)
                    .and_then(|opt| opt.as_ref().cloned())
                {
                    let spawn = self.resolve_spawn_target(
                        &object,
                        template_ref,
                        bridge_ref,
                        &ocl_info.time_and_location_info,
                        object_position,
                    )?;
                    match spawn {
                        ModuleEffectSpawn::Position(pos) => {
                            self.execute_ocl_at_position(handle_arc.as_ref(), &pos)?;
                        }
                        ModuleEffectSpawn::ParentObject => {
                            self.execute_ocl_on_object(
                                handle_arc.as_ref(),
                                &object,
                                &object_position,
                            )?;
                        }
                    }
                }
            }
        }
        Ok(UpdateSleepTime::None)
    }
}

// Implement BridgeBehaviorInterface
impl BridgeBehaviorInterface for BridgeBehavior {
    fn set_tower(&mut self, tower_type: BridgeTowerType, tower: Option<Arc<RwLock<GameObject>>>) {
        BridgeBehavior::set_tower(self, tower_type, tower);
    }

    fn get_tower_id(&self, tower_type: BridgeTowerType) -> ObjectID {
        BridgeBehavior::get_tower_id(self, tower_type)
    }

    fn create_scaffolding(&mut self) {
        let _ = BridgeBehavior::create_scaffolding(self);
    }

    fn remove_scaffolding(&mut self) {
        let _ = BridgeBehavior::remove_scaffolding(self);
    }

    fn is_scaffold_in_motion(&self) -> Bool {
        BridgeBehavior::is_scaffold_in_motion(self)
    }

    fn is_scaffold_present(&self) -> Bool {
        BridgeBehavior::is_scaffold_present(self)
    }
}

// Implement BehaviorModuleInterface
impl crate::modules::BehaviorModuleInterface for BridgeBehavior {
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
    fn get_damage(&mut self) -> Option<&mut dyn DamageModuleInterface> {
        Some(self)
    }
    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        Some(self)
    }
    fn get_bridge_behavior_interface(&mut self) -> Option<&mut dyn BridgeBehaviorInterface> {
        Some(self)
    }
}

/// Glue that binds the behavior to the module factory infrastructure.
pub struct BridgeBehaviorModule {
    behavior: BridgeBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<BridgeBehaviorModuleData>,
}

impl BridgeBehaviorModule {
    pub fn new(
        behavior: BridgeBehavior,
        module_name: &AsciiString,
        module_data: Arc<BridgeBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior(&self) -> &BridgeBehavior {
        &self.behavior
    }

    pub fn behavior_mut(&mut self) -> &mut BridgeBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for BridgeBehaviorModule {
    fn crc(&self, xfer: &mut dyn EngineXfer) -> Result<(), String> {
        self.behavior.crc(xfer).map_err(|err| err.to_string())
    }

    fn xfer(&mut self, xfer: &mut dyn EngineXfer) -> Result<(), String> {
        self.behavior.xfer(xfer).map_err(|err| err.to_string())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior
            .load_post_process()
            .map_err(|err| err.to_string())
    }
}

impl EngineModule for BridgeBehaviorModule {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ThingModuleData {
        self.module_data.as_ref()
    }

    fn on_object_created(&mut self) {}

    fn on_delete(&mut self) {}
}

unsafe impl Send for BridgeBehavior {}
unsafe impl Sync for BridgeBehavior {}

/// Helper function: Determine if old state is worse than new state (i.e., got repaired)
/// Matches C++ macro IS_CONDITION_WORSE from BridgeBehavior.cpp:637
fn is_condition_worse(old_state: BodyDamageType, new_state: BodyDamageType) -> bool {
    let old_index = damage_type_to_index(old_state);
    let new_index = damage_type_to_index(new_state);
    old_index > new_index
}

/// Convert BodyDamageType to array index
/// Matches C++ BODYDAMAGETYPE_COUNT ordering
fn damage_type_to_index(damage_type: BodyDamageType) -> usize {
    match damage_type {
        BodyDamageType::Pristine => BODY_PRISTINE,
        BodyDamageType::Damaged => 1,
        BodyDamageType::ReallyDamaged => 2,
        BodyDamageType::Rubble => 3,
        _ => BODY_PRISTINE,
    }
}

fn terrain_bridge_tower_type_from_index(index: usize) -> Option<crate::common::BridgeTowerType> {
    match index {
        0 => Some(crate::common::BridgeTowerType::From),
        1 => Some(crate::common::BridgeTowerType::To),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_behavior_creation() {
        // Test creation of bridge behavior
        // This would require mock implementations of dependencies
    }

    #[test]
    fn test_tower_management() {
        // Test setting and getting towers
    }

    #[test]
    fn test_scaffolding_system() {
        // Test scaffolding creation and removal
    }
}
