//! GhostObject - lightweight object proxy for partition/visibility tracking.
//!
//! This mirrors the original C++ GhostObject/GhostObjectManager implementation.

use crate::common::{Bool, Coord3D, Int, ObjectID, Real, INVALID_ID};
use crate::helpers::TheGameLogic;
use crate::object::{Object, PartitionData};
use game_engine::common::system::{GeometryType, Snapshotable, Xfer, XferMode, XferVersion};
use once_cell::sync::Lazy;
use std::sync::{Arc, RwLock};

fn geometry_type_to_u32(geo: GeometryType) -> u32 {
    match geo {
        GeometryType::Box => 0,
        GeometryType::Sphere => 1,
        GeometryType::Cylinder => 2,
    }
}

fn geometry_type_from_u32(value: u32) -> GeometryType {
    match value {
        1 => GeometryType::Sphere,
        2 => GeometryType::Cylinder,
        _ => GeometryType::Box,
    }
}

/// Lightweight proxy that mirrors a parent object's partition state.
#[derive(Debug)]
pub struct GhostObject {
    parent_angle: Real,
    parent_geometry_is_small: Bool,
    parent_geometry_major_radius: Real,
    parent_geometry_minor_radius: Real,
    parent_object: Option<Arc<RwLock<Object>>>,
    parent_geometry_type: GeometryType,
    parent_position: Coord3D,
    partition_data: Option<PartitionData>,
}

impl GhostObject {
    pub fn new() -> Self {
        Self {
            parent_angle: 0.0,
            parent_geometry_is_small: false,
            parent_geometry_major_radius: 0.0,
            parent_geometry_minor_radius: 0.0,
            parent_object: None,
            parent_geometry_type: GeometryType::Box,
            parent_position: Coord3D::ZERO,
            partition_data: None,
        }
    }

    pub fn set_parent(&mut self, parent: Option<Arc<RwLock<Object>>>) {
        self.parent_object = parent;
    }

    pub fn get_parent(&self) -> Option<Arc<RwLock<Object>>> {
        self.parent_object.clone()
    }

    pub fn set_partition_data(&mut self, data: Option<PartitionData>) {
        self.partition_data = data;
    }
}

impl Snapshotable for GhostObject {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        let mut parent_id = self
            .parent_object
            .as_ref()
            .and_then(|obj| obj.read().ok().map(|o| o.get_id()))
            .unwrap_or(INVALID_ID);
        xfer.xfer_u32(&mut parent_id).map_err(|e| e.to_string())?;

        if xfer.get_xfer_mode() == XferMode::Load {
            self.parent_object = TheGameLogic::find_object_by_id(parent_id);
            if parent_id != INVALID_ID && self.parent_object.is_none() {
                return Err("GhostObject::xfer unable to connect parent object".to_string());
            }
        }

        let mut geometry_type_raw = geometry_type_to_u32(self.parent_geometry_type);
        xfer.xfer_u32(&mut geometry_type_raw)
            .map_err(|e| e.to_string())?;
        self.parent_geometry_type = geometry_type_from_u32(geometry_type_raw);

        xfer.xfer_bool(&mut self.parent_geometry_is_small)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.parent_geometry_major_radius)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.parent_geometry_minor_radius)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.parent_angle)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.parent_position.x)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.parent_position.y)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.parent_position.z)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Default for GhostObject {
    fn default() -> Self {
        Self::new()
    }
}

/// Manager for ghost objects (used for partition and visibility tracking).
#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct GhostObjectManager {
    lock_ghost_objects: Bool,
    save_lock_ghost_objects: Bool,
    local_player: Int,
    ghost_objects: Vec<Arc<RwLock<GhostObject>>>,
}

impl GhostObjectManager {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            lock_ghost_objects: false,
            save_lock_ghost_objects: false,
            local_player: 0,
            ghost_objects: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        // C++ implementation is empty in this codebase.
    }

    pub fn add_ghost_object(
        &mut self,
        object: &Arc<RwLock<Object>>,
        partition_data: Option<PartitionData>,
    ) -> Option<Arc<RwLock<GhostObject>>> {
        // Respect the lock flag — C++ uses this during map border resizing.
        if self.lock_ghost_objects {
            return None;
        }

        let (position, angle, geometry_type, is_small, major_radius, minor_radius) = {
            match object.read() {
                Ok(obj) => {
                    let geom = obj.get_geometry_info();
                    (
                        *obj.get_position(),
                        obj.get_orientation(),
                        geom.get_geometry_type(),
                        geom.get_is_small(),
                        geom.get_major_radius(),
                        geom.get_minor_radius(),
                    )
                }
                Err(_) => return None,
            }
        };

        let ghost = GhostObject {
            parent_object: Some(object.clone()),
            parent_position: position,
            parent_angle: angle,
            parent_geometry_type: geometry_type,
            parent_geometry_is_small: is_small,
            parent_geometry_major_radius: major_radius,
            parent_geometry_minor_radius: minor_radius,
            partition_data,
        };

        let arc = Arc::new(RwLock::new(ghost));
        self.ghost_objects.push(arc.clone());
        Some(arc)
    }

    pub fn remove_ghost_object(&mut self, ghost: &Arc<RwLock<GhostObject>>) {
        // Find and remove by pointer identity (Arc::ptr_eq).
        self.ghost_objects
            .retain(|g| !Arc::ptr_eq(g, ghost));
    }

    pub fn update_orphaned_objects(&mut self, _player_index_list: &[Int]) {
        // Original C++ implementation is empty.
    }

    pub fn release_partition_data(&mut self) {
        // Original C++ implementation is empty.
    }

    pub fn restore_partition_data(&mut self) {
        // Original C++ implementation is empty.
    }

    pub fn set_local_player_index(&mut self, index: Int) {
        self.local_player = index;
    }

    pub fn get_local_player_index(&self) -> Int {
        self.local_player
    }

    pub fn lock_ghost_objects(&mut self, enable: Bool) {
        self.lock_ghost_objects = enable;
    }

    pub fn save_lock_ghost_objects(&mut self, enable: Bool) {
        self.save_lock_ghost_objects = enable;
    }
}

impl Snapshotable for GhostObjectManager {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.local_player)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub static THE_GHOST_OBJECT_MANAGER: Lazy<Arc<RwLock<GhostObjectManager>>> =
    Lazy::new(|| Arc::new(RwLock::new(GhostObjectManager::new())));
