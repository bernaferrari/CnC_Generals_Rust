////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Build Assistant System
//!
//! Singleton class that encapsulates common functions and rules that apply
//! to building structures and units. Handles construction validation, object
//! placement, terrain checking, and the selling process.
//!
//! Colin Day, February 2002
//! Rust conversion: 2025

use crate::common::ascii_string::AsciiString;
use crate::common::global_data;
use crate::common::system::kind_of::KindOfMask;
use once_cell::sync::OnceCell;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

/// Construction completion constant
pub const CONSTRUCTION_COMPLETE: i32 = -1;

/// Frame constants for selling
const FRAMES_TO_ALLOW_SCAFFOLD: f32 = 30.0 * 1.5; // Assuming 30 FPS (LOGICFRAMES_PER_SECOND)
const TOTAL_FRAMES_TO_SELL_OBJECT: f32 = 30.0 * 3.0;

/// 3D coordinate structure
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Backend hook for integrating build assistant actions with game logic.
pub trait BuildAssistantBackend: std::fmt::Debug + Send + Sync {
    fn build_object_now(
        &self,
        builder_id: Option<ObjectID>,
        template_name: &str,
        pos: &Coord3D,
        angle: f32,
        owning_player: u32,
    ) -> Option<ObjectID>;

    fn is_location_legal_to_build(
        &self,
        world_pos: &Coord3D,
        template_name: &str,
        angle: f32,
        options: LocalLegalToBuildOptions,
        builder_id: Option<ObjectID>,
        player_id: Option<u32>,
    ) -> LegalBuildCode;

    fn get_ground_height(&self, x: f32, y: f32) -> f32 {
        let _ = (x, y);
        0.0
    }

    fn find_sell_object(&self, id: ObjectID) -> Option<SellObjectSnapshot> {
        let _ = id;
        None
    }
    fn set_construction_percent(&self, id: ObjectID, percent: f32) {
        let _ = (id, percent);
    }
    fn set_sold_model_condition(&self, id: ObjectID) {
        let _ = id;
    }
    fn deposit_refund(&self, player_index: u32, amount: u32) {
        let _ = (player_index, amount);
    }
    fn cancel_and_refund_production(&self, id: ObjectID) {
        let _ = id;
    }
    fn destroy_object(&self, id: ObjectID) {
        let _ = id;
    }
    fn special_power_construction_matches(
        &self,
        builder_id: ObjectID,
        template_name: &str,
    ) -> bool {
        let _ = (builder_id, template_name);
        false
    }
    fn builder_is_script_disabled(&self, id: ObjectID) -> bool {
        let _ = id;
        false
    }
    fn builder_is_unpowered(&self, id: ObjectID) -> bool {
        let _ = id;
        false
    }

    fn can_build_more_of_type(&self, player_id: Option<u32>, template_name: &str) -> bool {
        let _ = (player_id, template_name);
        true
    }
    fn can_queue_create_unit(&self, builder_id: ObjectID, template_name: &str) -> CanMakeType {
        let _ = (builder_id, template_name);
        CanMakeType::Ok
    }
    fn calc_cost_to_build(&self, player_id: Option<u32>, template_name: &str) -> u32 {
        let _ = (player_id, template_name);
        0
    }
    fn player_money(&self, player_id: Option<u32>) -> u32 {
        let _ = player_id;
        u32::MAX
    }
    fn object_kind_of(&self, id: ObjectID) -> u128 {
        let _ = id;
        0
    }
    fn object_is_dead(&self, id: ObjectID) -> bool {
        let _ = id;
        false
    }
    fn objects_in_footprint(
        &self,
        pos: &Coord3D,
        template_name: &str,
        angle: f32,
    ) -> Vec<ObjectID> {
        let _ = (pos, template_name, angle);
        Vec::new()
    }
    fn remove_trees_and_props(&self, pos: &Coord3D, template_name: &str, angle: f32) {
        let _ = (pos, template_name, angle);
    }
    fn move_object_aside(&self, id: ObjectID, from: &Coord3D, radius: f32) -> bool {
        let _ = (id, from, radius);
        true
    }
    fn object_relationship_enemy(&self, id: ObjectID, player_id: Option<u32>) -> bool {
        let _ = (id, player_id);
        false
    }
    /// C++ BuildAssistant::sellObject calls contain->onSelling() at sell start.
    fn on_selling(&self, id: ObjectID) {
        let _ = id;
    }
}

/// Live object data used while selling.
#[derive(Debug, Clone)]
pub struct SellObjectSnapshot {
    pub construction_percent: f32,
    pub refund_value: u32,
    pub cost_to_build: u32,
    pub player_index: u32,
}

fn backend_cell() -> &'static Mutex<Option<Arc<dyn BuildAssistantBackend>>> {
    static BACKEND: OnceCell<Mutex<Option<Arc<dyn BuildAssistantBackend>>>> = OnceCell::new();
    BACKEND.get_or_init(|| Mutex::new(None))
}

pub fn set_build_assistant_backend(backend: Arc<dyn BuildAssistantBackend>) {
    let mut guard = backend_cell()
        .lock()
        .expect("Build assistant backend lock poisoned");
    *guard = Some(backend);
}

pub fn clear_build_assistant_backend() {
    let mut guard = backend_cell()
        .lock()
        .expect("Build assistant backend lock poisoned");
    *guard = None;
}

fn get_build_assistant_backend() -> Option<Arc<dyn BuildAssistantBackend>> {
    backend_cell()
        .lock()
        .expect("Build assistant backend lock poisoned")
        .clone()
}

impl Default for Coord3D {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

impl Coord3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn normalize(&mut self) {
        let len = self.length();
        if len > 0.0 {
            self.x /= len;
            self.y /= len;
            self.z /= len;
        }
    }
}

/// Object ID type
pub type ObjectID = u32;
pub const INVALID_ID: ObjectID = 0xFFFFFFFF;

/// Object sell information
#[derive(Debug, Clone)]
pub struct ObjectSellInfo {
    pub id: ObjectID,
    pub sell_frame: u32,
}

impl Default for ObjectSellInfo {
    fn default() -> Self {
        Self {
            id: INVALID_ID,
            sell_frame: 0,
        }
    }
}

/// Return codes for queries about being able to build
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CanMakeType {
    Ok,
    NoPrereq,
    NoMoney,
    FactoryIsDisabled,
    QueueFull,
    ParkingPlacesFull,
    MaxedOutForPlayer,
}

/// Return codes for queries about legal build locations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LegalBuildCode {
    Ok = 0,
    RestrictedTerrain,
    NotFlatEnough,
    ObjectsInTheWay,
    NoClearPath,
    Shroud,
    TooCloseToSupplies,
    GenericFailure,
}

// Options for location legal to build checks
bitflags::bitflags! {
    pub struct LocalLegalToBuildOptions: u32 {
        const TERRAIN_RESTRICTIONS = 0x00000001;
        const CLEAR_PATH = 0x00000002;
        const NO_OBJECT_OVERLAP = 0x00000004;
        const USE_QUICK_PATHFIND = 0x00000008;
        const SHROUD_REVEALED = 0x00000010;
        const NO_ENEMY_OBJECT_OVERLAP = 0x00000020;
        const IGNORE_STEALTHED = 0x00000040;
        const FAIL_STEALTHED_WITHOUT_FEEDBACK = 0x00000080;
    }
}

/// Function type for iterating over footprint samples
pub type IterateFootprintFunc = fn(&Coord3D, &mut dyn std::any::Any);

/// Information about tiled building placement
#[derive(Debug)]
pub struct TileBuildInfo {
    pub tiles_used: i32,
    pub positions: Vec<Coord3D>,
}

/// 3D region structure
#[derive(Debug, Clone)]
pub struct Region3D {
    pub lo: Coord3D,
    pub hi: Coord3D,
}

impl Region3D {
    pub fn is_in_region_no_z(&self, point: &Coord3D) -> bool {
        point.x >= self.lo.x && point.x <= self.hi.x && point.y >= self.lo.y && point.y <= self.hi.y
    }
}

/// Geometry information for objects
#[derive(Debug, Clone)]
pub struct GeometryInfo {
    pub geom_type: GeometryType,
    pub major_radius: f32,
    pub minor_radius: f32,
    pub height: f32,
}

/// Types of geometry
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GeometryType {
    Box,
    Sphere,
    Cylinder,
}

impl GeometryInfo {
    pub fn new(geom_type: GeometryType, major_radius: f32, minor_radius: f32, height: f32) -> Self {
        Self {
            geom_type,
            major_radius,
            minor_radius,
            height,
        }
    }

    pub fn get_bounding_circle_radius(&self) -> f32 {
        match self.geom_type {
            GeometryType::Box => (self.major_radius * self.major_radius
                + self.minor_radius * self.minor_radius)
                .sqrt(),
            GeometryType::Sphere | GeometryType::Cylinder => self.major_radius,
        }
    }
}

/// Build-assistant view of a thing template.
#[derive(Debug)]
pub struct ThingTemplate {
    pub name: AsciiString,
    pub geometry_info: GeometryInfo,
    pub line_build: bool,
    pub cost_to_build: u32,
    pub refund_value: u32,
}

impl ThingTemplate {
    pub fn new(name: &str) -> Self {
        Self {
            name: AsciiString::from(name),
            geometry_info: GeometryInfo::new(GeometryType::Box, 10.0, 10.0, 20.0),
            line_build: false,
            cost_to_build: 0,
            refund_value: 0,
        }
    }

    pub fn with_line_build(mut self, line_build: bool) -> Self {
        self.line_build = line_build;
        self
    }

    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    pub fn get_template_geometry_info(&self) -> &GeometryInfo {
        &self.geometry_info
    }

    pub fn is_line_build(&self) -> bool {
        self.line_build
    }
}

/// Build-assistant view of a player.
#[derive(Debug)]
pub struct Player {
    pub player_index: u32,
}

/// Build-assistant view of an object.
#[derive(Debug)]
pub struct Object {
    pub id: ObjectID,
    pub position: Coord3D,
    pub orientation: f32,
    /// C++ `Object::getCommandSetString()` when the host supplied one.
    pub command_set: Option<String>,
}

impl Object {
    pub fn new(id: ObjectID) -> Self {
        Self {
            id,
            position: Coord3D::default(),
            orientation: 0.0,
            command_set: None,
        }
    }
}

/// C++ isPossibleToMakeUnit: CommandSet must contain a Construct* button for `want`.
fn command_set_allows_construct(command_set: &str, want: &str) -> bool {
    command_set
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .filter(|tok| !tok.is_empty())
        .any(|tok| {
            tok.eq_ignore_ascii_case(want)
                || tok
                    .rsplit(|c| c == '_' || c == '/')
                    .next()
                    .is_some_and(|tail| tail.eq_ignore_ascii_case(want))
                || tok
                    .to_ascii_lowercase()
                    .contains(&want.to_ascii_lowercase())
        })
}

/// Build Assistant - manages construction and building validation
pub struct BuildAssistant {
    build_positions: Vec<Coord3D>,
    build_position_size: usize,
    sell_list: VecDeque<ObjectSellInfo>,
}

impl Default for BuildAssistant {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildAssistant {
    /// Create a new Build Assistant
    pub fn new() -> Self {
        Self {
            build_positions: Vec::new(),
            build_position_size: 0,
            sell_list: VecDeque::new(),
        }
    }

    /// Initialize the build assistant
    pub fn init(&mut self, max_line_build_objects: usize) {
        self.build_position_size = max_line_build_objects;
        self.build_positions = vec![Coord3D::default(); max_line_build_objects];
    }

    /// Reset the build assistant, clearing all data
    pub fn reset(&mut self) {
        self.sell_list.clear();
    }

    fn get_ground_height(&self, x: f32, y: f32) -> f32 {
        get_build_assistant_backend()
            .map(|backend| backend.get_ground_height(x, y))
            .unwrap_or(0.0)
    }

    /// Update the build assistant - processes selling objects
    pub fn update(&mut self, current_frame: u32) {
        let backend = get_build_assistant_backend();
        let mut items_to_remove = Vec::new();
        let sell_pct = global_data::read_safe()
            .map(|data| data.sell_percentage)
            .unwrap_or(1.0);

        for (index, sell_info) in self.sell_list.iter_mut().enumerate() {
            let Some(backend) = backend.as_ref() else {
                if current_frame.saturating_sub(sell_info.sell_frame)
                    >= TOTAL_FRAMES_TO_SELL_OBJECT as u32
                {
                    items_to_remove.push(index);
                }
                continue;
            };

            let Some(mut snap) = backend.find_sell_object(sell_info.id) else {
                items_to_remove.push(index);
                continue;
            };

            if current_frame.saturating_sub(sell_info.sell_frame) >= FRAMES_TO_ALLOW_SCAFFOLD as u32
            {
                let previous = snap.construction_percent;
                let next = previous - (100.0 / TOTAL_FRAMES_TO_SELL_OBJECT);
                backend.set_construction_percent(sell_info.id, next);
                snap.construction_percent = next;
                if previous > 0.0 && next <= 0.0 {
                    backend.set_sold_model_condition(sell_info.id);
                }
            }

            if snap.construction_percent <= -50.0 {
                let refund = if snap.refund_value != 0 {
                    snap.refund_value
                } else {
                    (snap.cost_to_build as f32 * sell_pct) as u32
                };
                backend.deposit_refund(snap.player_index, refund);
                backend.cancel_and_refund_production(sell_info.id);
                backend.destroy_object(sell_info.id);
                items_to_remove.push(index);
            }
        }

        for &index in items_to_remove.iter().rev() {
            self.sell_list.remove(index);
        }
    }

    /// Build an object immediately at the specified location
    pub fn build_object_now(
        &self,
        constructor_object: Option<&Object>,
        what: &ThingTemplate,
        pos: &Coord3D,
        angle: f32,
        owning_player: &Player,
    ) -> Option<Object> {
        if let Some(backend) = get_build_assistant_backend() {
            let builder_id = constructor_object.map(|obj| obj.id);
            if let Some(id) = backend.build_object_now(
                builder_id,
                what.get_name().as_str(),
                pos,
                angle,
                owning_player.player_index,
            ) {
                return Some(Object {
                    id,
                    position: *pos,
                    orientation: angle,
                    command_set: None,
                });
            }
            return None;
        }

        None
    }

    /// Build a line of objects from start to end
    pub fn build_object_line_now(
        &self,
        constructor_object: Option<&Object>,
        what: &ThingTemplate,
        start: &Coord3D,
        end: &Coord3D,
        angle: f32,
        owning_player: &Player,
    ) {
        let object_size = what.get_template_geometry_info().major_radius * 2.0;
        let max_objects = self.build_position_size as i32;

        if let Some(tile_info) = self.build_tiled_locations(
            what,
            angle,
            start,
            end,
            object_size,
            max_objects,
            constructor_object,
        ) {
            for position in &tile_info.positions {
                self.build_object_now(constructor_object, what, position, angle, owning_player);
            }
        }
    }

    /// Check if a location is legal to build at
    pub fn is_location_legal_to_build(
        &self,
        world_pos: &Coord3D,
        build: &ThingTemplate,
        angle: f32,
        options: LocalLegalToBuildOptions,
        builder_object: Option<&Object>,
        player: Option<&Player>,
    ) -> LegalBuildCode {
        if let Some(backend) = get_build_assistant_backend() {
            let builder_id = builder_object.map(|obj| obj.id);
            let player_id = player.map(|p| p.player_index);
            return backend.is_location_legal_to_build(
                world_pos,
                build.get_name().as_str(),
                angle,
                options,
                builder_id,
                player_id,
            );
        }

        LegalBuildCode::GenericFailure
    }

    /// Iterate over the footprint of a building
    pub fn iterate_footprint(
        &self,
        build: &ThingTemplate,
        build_orientation: f32,
        world_pos: &Coord3D,
        sample_resolution: f32,
        func: IterateFootprintFunc,
        func_user_data: &mut dyn std::any::Any,
    ) {
        let geometry = build.get_template_geometry_info();

        let (half_width, half_height) = match geometry.geom_type {
            GeometryType::Box => (geometry.major_radius, geometry.minor_radius),
            GeometryType::Sphere | GeometryType::Cylinder => {
                let radius = geometry.get_bounding_circle_radius();
                (radius, radius)
            }
        };

        let mut y = -half_height;
        while y < half_height + sample_resolution {
            if y > half_height {
                y = half_height;
            }

            let mut x = -half_width;
            while x < half_width + sample_resolution {
                if x > half_width {
                    x = half_width;
                }

                // Transform to world coordinates
                let cos_angle = build_orientation.cos();
                let sin_angle = build_orientation.sin();

                let world_x = world_pos.x + x * cos_angle - y * sin_angle;
                let world_y = world_pos.y + x * sin_angle + y * cos_angle;

                // For circular geometries, check if we're within the circle
                if matches!(
                    geometry.geom_type,
                    GeometryType::Sphere | GeometryType::Cylinder
                ) {
                    let distance = (x * x + y * y).sqrt();
                    if distance > half_width {
                        x += sample_resolution;
                        continue;
                    }
                }

                let sample_point =
                    Coord3D::new(world_x, world_y, self.get_ground_height(world_x, world_y));
                func(&sample_point, func_user_data);

                x += sample_resolution;
            }
            y += sample_resolution;
        }
    }

    /// Build tiled locations for line building (like walls)
    pub fn build_tiled_locations(
        &self,
        thing_being_tiled: &ThingTemplate,
        angle: f32,
        start: &Coord3D,
        end: &Coord3D,
        tiling_size: f32,
        max_tiles: i32,
        builder_object: Option<&Object>,
    ) -> Option<TileBuildInfo> {
        let mut placement_vector = Coord3D::new(end.x - start.x, end.y - start.y, 0.0);

        let placement_length = placement_vector.length();
        let mut tiles_needed = (placement_length / tiling_size) as i32 + 1;

        if tiles_needed > max_tiles {
            tiles_needed = max_tiles;
        }

        placement_vector.normalize();

        let mut positions = Vec::with_capacity(tiles_needed as usize);
        positions.push(*start);

        for i in 1..tiles_needed {
            let x = placement_vector.x * (tiling_size * i as f32) + start.x;
            let y = placement_vector.y * (tiling_size * i as f32) + start.y;
            let pos = Coord3D::new(x, y, self.get_ground_height(x, y));

            // Check if this position is legal to build at
            if self.is_location_legal_to_build(
                &pos,
                thing_being_tiled,
                angle,
                LocalLegalToBuildOptions::USE_QUICK_PATHFIND
                    | LocalLegalToBuildOptions::TERRAIN_RESTRICTIONS
                    | LocalLegalToBuildOptions::CLEAR_PATH
                    | LocalLegalToBuildOptions::NO_OBJECT_OVERLAP
                    | LocalLegalToBuildOptions::SHROUD_REVEALED,
                builder_object,
                None,
            ) != LegalBuildCode::Ok
            {
                break;
            }

            positions.push(pos);
        }

        Some(TileBuildInfo {
            tiles_used: positions.len() as i32,
            positions,
        })
    }

    /// C++ `BuildAssistant::addBibs` — highlight nearby immobile structures.
    /// Live host bibs blockers from `legal_build_code_at_for_builder`; this leftover
    /// path is unused on the live tick (empty registry / skipped update).
    pub fn add_bibs(&self, _world_pos: &Coord3D, _build: &ThingTemplate) {
        // Backend hook: live host paints object bibs via TerrainVisual.
    }

    /// Check if a template is for line building (walls, etc.)
    pub fn is_line_build_template(&self, template: &ThingTemplate) -> bool {
        template.is_line_build()
    }

    /// Check if it's possible to make a unit (ignoring money)
    pub fn is_possible_to_make_unit(
        &self,
        builder: &Object,
        what_to_build: &ThingTemplate,
    ) -> bool {
        if builder.id == INVALID_ID || what_to_build.get_name().is_empty() {
            return false;
        }
        // C++ scans the builder CommandSet for UNIT_BUILD / DOZER_CONSTRUCT.
        if let Some(command_set) = builder.command_set.as_deref() {
            let want = what_to_build.get_name().as_str();
            return command_set_allows_construct(command_set, want);
        }
        true
    }

    /// Check if a unit can be made (including money check)
    pub fn can_make_unit(&self, builder: &Object, what_to_build: &ThingTemplate) -> CanMakeType {
        if builder.id == INVALID_ID || what_to_build.get_name().is_empty() {
            return CanMakeType::NoPrereq;
        }

        if let Some(backend) = get_build_assistant_backend() {
            if backend.builder_is_script_disabled(builder.id)
                || backend.builder_is_unpowered(builder.id)
            {
                return CanMakeType::FactoryIsDisabled;
            }
            if backend
                .special_power_construction_matches(builder.id, what_to_build.get_name().as_str())
            {
                return CanMakeType::Ok;
            }
            if !backend.can_build_more_of_type(None, what_to_build.get_name().as_str()) {
                return CanMakeType::MaxedOutForPlayer;
            }
        }

        if !self.is_possible_to_make_unit(builder, what_to_build) {
            return CanMakeType::NoPrereq;
        }

        if let Some(backend) = get_build_assistant_backend() {
            let queued =
                backend.can_queue_create_unit(builder.id, what_to_build.get_name().as_str());
            if queued != CanMakeType::Ok {
                return queued;
            }
            let cost = backend.calc_cost_to_build(None, what_to_build.get_name().as_str());
            if cost > 0 && cost > backend.player_money(None) {
                return CanMakeType::NoMoney;
            }
        }

        CanMakeType::Ok
    }

    /// Start the selling process for an object
    pub fn sell_object(&mut self, object: &Object, current_frame: u32) {
        for sell_info in &self.sell_list {
            if sell_info.id == object.id {
                return;
            }
        }
        self.sell_list.push_front(ObjectSellInfo {
            id: object.id,
            sell_frame: current_frame,
        });
        // C++ BuildAssistant.cpp:1542-1547 — notify contain at sell start.
        if let Some(backend) = get_build_assistant_backend() {
            backend.on_selling(object.id);
        }
    }

    /// Check if an object is removable for construction
    pub fn is_removable_for_construction(&self, object: &Object) -> bool {
        if object.id == INVALID_ID {
            return false;
        }
        let kind = get_build_assistant_backend()
            .map(|backend| KindOfMask::from_bits_truncate(backend.object_kind_of(object.id)))
            .unwrap_or_else(KindOfMask::empty);
        if kind.contains(KindOfMask::INERT) {
            return false;
        }
        if kind.contains(KindOfMask::SHRUBBERY) || kind.contains(KindOfMask::CLEARED_BY_BUILD) {
            return true;
        }
        if let Some(backend) = get_build_assistant_backend() {
            if backend.object_is_dead(object.id) {
                return true;
            }
        }
        false
    }

    /// C++ BuildAssistant::clearRemovableForConstruction
    pub fn clear_removable_for_construction(
        &self,
        what_to_build: &ThingTemplate,
        pos: &Coord3D,
        angle: f32,
    ) {
        let Some(backend) = get_build_assistant_backend() else {
            return;
        };
        for id in backend.objects_in_footprint(pos, what_to_build.get_name().as_str(), angle) {
            let kind = KindOfMask::from_bits_truncate(backend.object_kind_of(id));
            if kind.contains(KindOfMask::ALWAYS_SELECTABLE) {
                continue;
            }
            let probe = Object {
                id,
                position: *pos,
                orientation: angle,
                command_set: None,
            };
            if self.is_removable_for_construction(&probe) {
                backend.destroy_object(id);
            }
        }
        backend.remove_trees_and_props(pos, what_to_build.get_name().as_str(), angle);
    }

    /// C++ BuildAssistant::moveObjectsForConstruction
    pub fn move_objects_for_construction(
        &self,
        what_to_build: &ThingTemplate,
        pos: &Coord3D,
        angle: f32,
        player: Option<&Player>,
    ) -> bool {
        let Some(backend) = get_build_assistant_backend() else {
            return true;
        };
        let geom = what_to_build.get_template_geometry_info();
        let mut radius =
            (geom.major_radius * geom.major_radius + geom.minor_radius * geom.minor_radius).sqrt();
        radius *= 1.4;
        let player_id = player.map(|p| p.player_index);
        let mut any_unmovables = false;
        for id in backend.objects_in_footprint(pos, what_to_build.get_name().as_str(), angle) {
            let kind = KindOfMask::from_bits_truncate(backend.object_kind_of(id));
            if kind.contains(KindOfMask::MINE) || kind.contains(KindOfMask::INERT) {
                continue;
            }
            let probe = Object {
                id,
                position: *pos,
                orientation: angle,
                command_set: None,
            };
            if kind.contains(KindOfMask::ALWAYS_SELECTABLE)
                || self.is_removable_for_construction(&probe)
            {
                continue;
            }
            if backend.object_relationship_enemy(id, player_id) {
                any_unmovables = true;
                continue;
            }
            if !backend.move_object_aside(id, pos, radius) {
                any_unmovables = true;
            }
        }
        !any_unmovables
    }

    /// Get the build positions array
    pub fn get_build_locations(&self) -> &[Coord3D] {
        &self.build_positions
    }

    /// Get the sell list for serialization
    pub fn get_sell_list(&self) -> &VecDeque<ObjectSellInfo> {
        &self.sell_list
    }
}

/// Global build assistant instance
static BUILD_ASSISTANT: OnceCell<Mutex<BuildAssistant>> = OnceCell::new();

/// Initialize the global build assistant
pub fn init_build_assistant() {
    if BUILD_ASSISTANT.get().is_none() {
        let _ = BUILD_ASSISTANT.set(Mutex::new(BuildAssistant::new()));
    } else if let Some(cell) = BUILD_ASSISTANT.get() {
        if let Ok(mut guard) = cell.lock() {
            *guard = BuildAssistant::new();
        }
    }
}

/// Get reference to the global build assistant
pub fn get_build_assistant() -> Option<MutexGuard<'static, BuildAssistant>> {
    BUILD_ASSISTANT
        .get()
        .map(|cell| cell.lock().expect("BuildAssistant mutex poisoned"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_assistant_creation() {
        let assistant = BuildAssistant::new();
        assert_eq!(assistant.build_position_size, 0);
        assert_eq!(assistant.sell_list.len(), 0);
    }

    #[test]
    fn test_coord3d() {
        let mut coord = Coord3D::new(3.0, 4.0, 0.0);
        assert_eq!(coord.length(), 5.0);

        coord.normalize();
        assert!((coord.length() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_sell_object() {
        let mut assistant = BuildAssistant::new();
        let object = Object::new(123);

        assistant.sell_object(&object, 100);
        assert_eq!(assistant.sell_list.len(), 1);
        assert_eq!(assistant.sell_list[0].id, 123);
        assert_eq!(assistant.sell_list[0].sell_frame, 100);
    }

    #[test]
    fn test_is_location_legal_to_build() {
        let assistant = BuildAssistant::new();
        let template = ThingTemplate::new("TestBuilding");
        let pos = Coord3D::new(0.0, 0.0, 0.0);

        let result = assistant.is_location_legal_to_build(
            &pos,
            &template,
            0.0,
            LocalLegalToBuildOptions::TERRAIN_RESTRICTIONS,
            None,
            None,
        );

        assert_eq!(result, LegalBuildCode::GenericFailure);
    }

    #[test]
    fn test_is_line_build_template_uses_template_flag() {
        let assistant = BuildAssistant::new();
        let normal = ThingTemplate::new("AmericaPowerPlant");
        let wall = ThingTemplate::new("ChinaWallSegment").with_line_build(true);

        assert!(!assistant.is_line_build_template(&normal));
        assert!(assistant.is_line_build_template(&wall));
    }

    #[test]
    fn test_build_tiled_locations() {
        let assistant = BuildAssistant::new();
        let template = ThingTemplate::new("Wall");
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let end = Coord3D::new(100.0, 0.0, 0.0);

        if let Some(tile_info) =
            assistant.build_tiled_locations(&template, 0.0, &start, &end, 10.0, 20, None)
        {
            assert!(tile_info.tiles_used > 0);
            assert_eq!(tile_info.positions[0], start);
        }
    }
}
