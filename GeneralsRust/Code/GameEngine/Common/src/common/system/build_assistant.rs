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

/// Mock thing template for compilation
#[derive(Debug)]
pub struct ThingTemplate {
    pub name: AsciiString,
    pub geometry_info: GeometryInfo,
}

impl ThingTemplate {
    pub fn new(name: &str) -> Self {
        Self {
            name: AsciiString::from(name),
            geometry_info: GeometryInfo::new(GeometryType::Box, 10.0, 10.0, 20.0),
        }
    }

    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    pub fn get_template_geometry_info(&self) -> &GeometryInfo {
        &self.geometry_info
    }
}

/// Mock player structure
#[derive(Debug)]
pub struct Player {
    pub player_index: u32,
}

/// Mock object structure
#[derive(Debug)]
pub struct Object {
    pub id: ObjectID,
    pub position: Coord3D,
    pub orientation: f32,
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

    /// Update the build assistant - processes selling objects
    pub fn update(&mut self, current_frame: u32) {
        let mut items_to_remove = Vec::new();

        for (index, sell_info) in self.sell_list.iter_mut().enumerate() {
            // Mock object lookup - in real implementation this would find the object
            // For now we'll just simulate the selling process

            if current_frame - sell_info.sell_frame >= FRAMES_TO_ALLOW_SCAFFOLD as u32 {
                // Simulate construction percent decreasing
                // In real implementation, this would modify the actual object
            }

            // Check if sell process is complete
            if current_frame - sell_info.sell_frame >= TOTAL_FRAMES_TO_SELL_OBJECT as u32 {
                items_to_remove.push(index);
            }
        }

        // Remove completed sell items
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

                let sample_point = Coord3D::new(world_x, world_y, 0.0); // Z would be ground height
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
            let pos = Coord3D::new(
                placement_vector.x * (tiling_size * i as f32) + start.x,
                placement_vector.y * (tiling_size * i as f32) + start.y,
                0.0, // Would be ground height in real implementation
            );

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

    /// Check if a template is for line building (walls, etc.)
    pub fn is_line_build_template(&self, _template: &ThingTemplate) -> bool {
        // Mock implementation - would check template flags
        false
    }

    /// Check if it's possible to make a unit (ignoring money)
    pub fn is_possible_to_make_unit(
        &self,
        _builder: &Object,
        _what_to_build: &ThingTemplate,
    ) -> bool {
        false
    }

    /// Check if a unit can be made (including money check)
    pub fn can_make_unit(&self, builder: &Object, what_to_build: &ThingTemplate) -> CanMakeType {
        if !self.is_possible_to_make_unit(builder, what_to_build) {
            return CanMakeType::NoPrereq;
        }

        CanMakeType::Ok
    }

    /// Start the selling process for an object
    pub fn sell_object(&mut self, object: &Object, current_frame: u32) {
        // Check if object is already being sold
        for sell_info in &self.sell_list {
            if sell_info.id == object.id {
                return; // Already being sold
            }
        }

        // Add to sell list
        let sell_info = ObjectSellInfo {
            id: object.id,
            sell_frame: current_frame,
        };

        self.sell_list.push_front(sell_info);
    }

    /// Check if an object is removable for construction
    pub fn is_removable_for_construction(&self, _object: &Object) -> bool {
        // Mock implementation - would check object types (shrubbery, debris, etc.)
        false
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
        let object = Object {
            id: 123,
            position: Coord3D::new(0.0, 0.0, 0.0),
            orientation: 0.0,
        };

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

        assert_eq!(result, LegalBuildCode::Ok);
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
