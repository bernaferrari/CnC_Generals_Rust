//! Supply Warehouse Dock Update Module
//!
//! Handles supply warehouse docking where supply trucks pick up or deliver supply crates.
//! Supply warehouses store a limited number of boxes and can be destroyed when empty.
//!
//! Original C++ location: GameLogic/Module/SupplyWarehouseDockUpdate.h/.cpp
//! Original C++ Author: Graham Smallwood, Feb 2002
//! Rust conversion: 2025

use crate::common::*;
use crate::modules::{BehaviorModule, BehaviorModuleInterface, DockUpdateInterface};
use crate::object::Object;
use crate::GameLogicRandomValueReal;
use game_engine::common::global_data;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, SupplyWarehouseDockInterface};
use std::sync::{Arc, RwLock};

/// Supply warehouse dock configuration data
#[derive(Debug, Clone)]
pub struct SupplyWarehouseDockUpdateData {
    /// Base dock data
    pub base: super::DockUpdateData,
    /// Starting number of supply boxes
    pub starting_boxes: i32,
    /// Whether to delete warehouse when empty
    pub delete_when_empty: bool,
}

impl SupplyWarehouseDockUpdateData {
    pub fn new() -> Self {
        Self {
            base: super::DockUpdateData::default(),
            starting_boxes: 1,
            delete_when_empty: false,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SUPPLY_WAREHOUSE_DOCK_UPDATE_FIELDS)
    }
}

impl Default for SupplyWarehouseDockUpdateData {
    fn default() -> Self {
        Self::new()
    }
}

impl Snapshotable for SupplyWarehouseDockUpdateData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

crate::impl_legacy_module_data_via_base!(SupplyWarehouseDockUpdateData, base);

fn parse_starting_boxes(
    _ini: &mut INI,
    data: &mut SupplyWarehouseDockUpdateData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.starting_boxes = INI::parse_int(token)?;
    Ok(())
}

fn parse_delete_when_empty(
    _ini: &mut INI,
    data: &mut SupplyWarehouseDockUpdateData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.delete_when_empty = INI::parse_bool(token)?;
    Ok(())
}

const SUPPLY_WAREHOUSE_DOCK_UPDATE_FIELDS: &[FieldParse<SupplyWarehouseDockUpdateData>] = &[
    FieldParse {
        token: "StartingBoxes",
        parse: parse_starting_boxes,
    },
    FieldParse {
        token: "DeleteWhenEmpty",
        parse: parse_delete_when_empty,
    },
];

/// Supply warehouse dock module
///
/// This dock handles supply truck interactions with supply warehouses.
/// Supply trucks pick up boxes from the warehouse, which has a limited capacity.
/// When empty, the warehouse can optionally be destroyed automatically.
///
/// # C++ Behavior Match
/// - Tracks number of boxes stored
/// - Can be crippled (disabled)
/// - Optionally self-destructs when empty
/// - Supplies money directly to trucks
#[derive(Debug)]
pub struct SupplyWarehouseDockUpdate {
    /// Base dock functionality
    base: super::DockUpdate,
    /// Warehouse configuration
    data: SupplyWarehouseDockUpdateData,
    /// Current number of boxes stored
    boxes_stored: i32,
    /// Whether the warehouse is crippled (disabled)
    is_crippled: bool,
}

impl SupplyWarehouseDockUpdate {
    /// Create a new supply warehouse dock
    pub fn new(
        data: SupplyWarehouseDockUpdateData,
        owner_id: ObjectID,
        owner_position: &Coord3D,
    ) -> Self {
        let base = super::DockUpdate::new(data.base.clone(), owner_id, owner_position);
        let starting_boxes = data.starting_boxes;

        Self {
            base,
            data,
            boxes_stored: starting_boxes,
            is_crippled: false,
        }
    }

    /// Get number of boxes currently stored
    pub fn get_boxes_stored(&self) -> i32 {
        self.boxes_stored
    }

    /// Set the number of boxes stored (used by create modules).
    pub fn set_boxes_stored(&mut self, boxes: i32) {
        self.boxes_stored = boxes.max(0);
        self.update_drawable_supply_status();
    }

    /// Set the cash value for this warehouse
    pub fn set_cash_value(&mut self, cash_value: i32) {
        let base_value = global_data::read_safe()
            .map(|data| data.base_value_per_supply_box.max(1))
            .unwrap_or(1);
        let boxes = (cash_value as f32 / base_value as f32).ceil() as i32;
        self.boxes_stored = boxes.max(0);
        self.update_drawable_supply_status();
    }

    fn update_drawable_supply_status(&self) {
        let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.base.owner_id())
        else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        let Some(drawable) = owner_guard.get_drawable() else {
            return;
        };
        let Ok(mut drawable_guard) = drawable.write() else {
            return;
        };
        drawable_guard.update_supply_status(self.data.starting_boxes, self.boxes_stored);
    }

    /// Handle supply truck docking and loading.
    fn perform_supply_transfer(&mut self, docker: &Arc<RwLock<Object>>) -> Result<bool, String> {
        if self.is_crippled || self.boxes_stored == 0 {
            return Ok(false);
        }

        let owner = crate::helpers::TheGameLogic::find_object_by_id(self.base.owner_id())
            .ok_or_else(|| "SupplyWarehouseDock: missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "SupplyWarehouseDock: owner lock")?;
        let docker_guard = docker
            .read()
            .map_err(|_| "SupplyWarehouseDock: docker lock")?;

        let close_enough_sqr = (docker_guard
            .get_geometry_info()
            .get_bounding_circle_radius()
            * 2.0)
            .powi(2);
        let cur_dist_sqr = crate::helpers::ThePartitionManager::get_distance_squared(
            &docker_guard,
            &owner_guard,
            FROM_BOUNDING_SPHERE_2D,
        );
        if cur_dist_sqr > close_enough_sqr {
            let mut new_pos = *docker_guard.get_position();
            let range = 0.4 * crate::path::PATHFIND_CELL_SIZE_F;
            new_pos.x += GameLogicRandomValueReal!(-range, range);
            new_pos.y += GameLogicRandomValueReal!(-range, range);
            drop(docker_guard);
            if let Ok(mut docker_write) = docker.write() {
                let _ = docker_write.set_position(&new_pos);
            }
            return Ok(false);
        }

        drop(owner_guard);
        drop(docker_guard);

        self.boxes_stored -= 1;

        let mut gained = false;
        if let Ok(docker_write) = docker.write() {
            if let Some(ai) = docker_write.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    if let Some(truck) = ai_guard.get_supply_truck_ai_interface_mut() {
                        gained = truck.gain_one_box(self.boxes_stored);
                    }
                }
            }
        }

        if gained {
            if self.boxes_stored == 0 && self.data.delete_when_empty {
                if let Err(err) =
                    crate::helpers::TheGameLogic::destroy_object_by_id(self.base.owner_id())
                {
                    log::warn!(
                        "SupplyWarehouseDockUpdate: failed to destroy empty warehouse {}: {}",
                        self.base.owner_id(),
                        err
                    );
                }
                return Ok(false);
            }
            self.update_drawable_supply_status();
            return Ok(true);
        }

        self.boxes_stored += 1;
        Ok(false)
    }
}

impl BehaviorModuleInterface for SupplyWarehouseDockUpdate {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update base dock
        self.base.update()?;
        Ok(())
    }

    fn get_module_name(&self) -> &str {
        "SupplyWarehouseDockUpdate"
    }

    fn get_interface_mask() -> u32 {
        0x00000004 // DOCK_UPDATE interface
    }

    fn get_dock_update_interface(&mut self) -> Option<&mut dyn DockUpdateInterface> {
        Some(self)
    }
}

impl BehaviorModule for SupplyWarehouseDockUpdate {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.init()?;
        self.update_drawable_supply_status();
        log::info!(
            "SupplyWarehouseDockUpdate initialized with {} boxes",
            self.boxes_stored
        );
        Ok(())
    }

    fn on_destroy(&mut self) {
        crate::resource::remove_supply_warehouse(self.base.owner_id());
        self.base.on_destroy();
        log::info!("SupplyWarehouseDockUpdate destroyed");
    }
}

impl DockUpdateInterface for SupplyWarehouseDockUpdate {
    fn is_dock_open(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // Warehouse is open if not crippled and has boxes
        Ok(!self.is_crippled && self.boxes_stored > 0)
    }

    fn supply_warehouse_boxes_stored(&self) -> Option<i32> {
        Some(self.get_boxes_stored())
    }

    fn set_dock_open(&mut self, open: Bool) {
        self.base.set_dock_open(open);
    }

    fn cancel_dock(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.cancel_dock(obj_id)
    }

    fn reserve_approach_position(
        &mut self,
        obj_id: ObjectID,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .reserve_approach_position(obj_id, goal_pos, approach_pos)
    }

    fn advance_approach_position(
        &mut self,
        obj_id: ObjectID,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .advance_approach_position(obj_id, goal_pos, approach_pos)
    }

    fn is_clear_to_advance(
        &self,
        obj_id: ObjectID,
        approach_position: i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_clear_to_advance(obj_id, approach_position)
    }

    fn on_approach_reached(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_approach_reached(obj_id)
    }

    fn is_clear_to_enter(
        &self,
        obj_id: ObjectID,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // Can only enter if not crippled and has boxes
        if self.is_crippled || self.boxes_stored <= 0 {
            return Ok(false);
        }
        self.base.is_clear_to_enter(obj_id)
    }

    fn get_enter_position(
        &self,
        obj_id: ObjectID,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.get_enter_position(obj_id, goal_pos)
    }

    fn on_enter_reached(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_enter_reached(obj_id)
    }

    fn get_dock_position(
        &self,
        obj_id: ObjectID,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.get_dock_position(obj_id, goal_pos)
    }

    fn on_dock_reached(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_dock_reached(obj_id)
    }

    fn action(
        &mut self,
        obj_id: ObjectID,
        _drone_id: Option<ObjectID>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // Perform supply transfer to truck
        {
            let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
            else {
                return Ok(false);
            };
            self.perform_supply_transfer(&obj)
        }
        .map_err(|e| e.into())
    }

    fn get_exit_position(
        &self,
        obj_id: ObjectID,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.get_exit_position(obj_id, goal_pos)
    }

    fn on_exit_reached(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_exit_reached(obj_id)
    }

    fn is_allow_passthrough_type(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.base.is_allow_passthrough_type()
    }

    fn is_rally_point_after_dock_type(
        &self,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(false) // Supply trucks don't use rally points after docking
    }

    fn set_dock_crippled(
        &mut self,
        crippled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if crippled {
            let active_id = self.base.active_docker_id();
            if active_id != INVALID_ID {
                if let Some(victim) = crate::helpers::TheGameLogic::find_object_by_id(active_id) {
                    if let Ok(mut victim_guard) = victim.write() {
                        if self.base.docker_inside() {
                            if !victim_guard.is_using_airborne_locomotor() {
                                victim_guard.kill(None, None);
                            }
                        } else if let Some(ai) = victim_guard.get_ai_update_interface() {
                            if let Ok(mut ai_guard) = ai.lock() {
                                if let Some(truck) = ai_guard.get_supply_truck_ai_interface_mut() {
                                    victim_guard.ai_idle();
                                    truck.set_force_wanting_state(true);
                                }
                            }
                        }
                    }
                }
            }
        }

        self.is_crippled = crippled;
        self.base.set_dock_crippled(crippled)?;
        Ok(())
    }
}

/// Glue that exposes SupplyWarehouseDockUpdate through the common Module trait.
pub struct SupplyWarehouseDockUpdateModule {
    behavior: SupplyWarehouseDockUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<SupplyWarehouseDockUpdateData>,
}

impl SupplyWarehouseDockUpdateModule {
    pub fn new(
        behavior: SupplyWarehouseDockUpdate,
        module_name: &AsciiString,
        module_data: Arc<SupplyWarehouseDockUpdateData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior(&self) -> &SupplyWarehouseDockUpdate {
        &self.behavior
    }

    pub fn behavior_mut(&mut self) -> &mut SupplyWarehouseDockUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for SupplyWarehouseDockUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1).map_err(|err| {
            format!("SupplyWarehouseDockUpdateModule::xfer version failed: {err}")
        })?;
        self.behavior.base.xfer(xfer)?;
        xfer.xfer_int(&mut self.behavior.boxes_stored)
            .map_err(|err| {
                format!("SupplyWarehouseDockUpdateModule::xfer boxes_stored failed: {err}")
            })
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.base.load_post_process()?;
        self.behavior.update_drawable_supply_status();
        Ok(())
    }
}

impl Module for SupplyWarehouseDockUpdateModule {
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
        game_engine::common::thing::module::ModuleData::get_module_tag_name_key(
            self.module_data.as_ref(),
        )
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }

    fn get_supply_warehouse_dock_interface(
        &mut self,
    ) -> Option<&mut dyn SupplyWarehouseDockInterface> {
        Some(self)
    }
}

impl SupplyWarehouseDockInterface for SupplyWarehouseDockUpdateModule {
    fn boxes_stored(&self) -> i32 {
        self.behavior.get_boxes_stored()
    }

    fn set_cash_value(&mut self, cash_value: i32) {
        self.behavior.set_cash_value(cash_value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supply_warehouse_dock_creation() {
        let data = SupplyWarehouseDockUpdateData {
            starting_boxes: 10,
            delete_when_empty: true,
            ..Default::default()
        };

        let pos = Coord3D::new(0.0, 0.0, 0.0);
        let dock = SupplyWarehouseDockUpdate::new(data, 1, &pos);

        assert_eq!(dock.get_boxes_stored(), 10);
        assert!(!dock.is_crippled);
    }

    #[test]
    fn test_supply_warehouse_boxes_exposed_through_dock_interface() {
        let data = SupplyWarehouseDockUpdateData {
            starting_boxes: 7,
            ..Default::default()
        };

        let pos = Coord3D::new(0.0, 0.0, 0.0);
        let dock = SupplyWarehouseDockUpdate::new(data, 1, &pos);

        assert_eq!(dock.supply_warehouse_boxes_stored(), Some(7));
    }

    #[test]
    fn test_supply_warehouse_box_tracking() {
        let mut data = SupplyWarehouseDockUpdateData::default();
        data.starting_boxes = 5;

        let pos = Coord3D::new(0.0, 0.0, 0.0);
        let mut dock = SupplyWarehouseDockUpdate::new(data, 1, &pos);

        assert_eq!(dock.get_boxes_stored(), 5);

        // Simulate taking boxes
        dock.boxes_stored -= 1;
        assert_eq!(dock.get_boxes_stored(), 4);
    }

    #[test]
    fn test_supply_warehouse_set_cash_value() {
        let data = SupplyWarehouseDockUpdateData {
            starting_boxes: 10,
            ..Default::default()
        };

        let pos = Coord3D::new(0.0, 0.0, 0.0);
        let mut dock = SupplyWarehouseDockUpdate::new(data, 1, &pos);

        dock.set_cash_value(5000); // $5000 total
        assert!(dock.boxes_stored > 0);
    }

    #[test]
    fn test_supply_warehouse_is_dock_open() {
        let data = SupplyWarehouseDockUpdateData::default();
        let pos = Coord3D::new(0.0, 0.0, 0.0);
        let mut dock = SupplyWarehouseDockUpdate::new(data, 1, &pos);

        // Should be open with boxes
        assert!(dock.is_dock_open().unwrap());

        // Should be closed when crippled
        dock.is_crippled = true;
        assert!(!dock.is_dock_open().unwrap());

        // Should be closed when empty
        dock.is_crippled = false;
        dock.boxes_stored = 0;
        assert!(!dock.is_dock_open().unwrap());
    }

    #[test]
    fn test_supply_warehouse_module_exposes_typed_interface() {
        let data = Arc::new(SupplyWarehouseDockUpdateData {
            starting_boxes: 4,
            ..Default::default()
        });
        let pos = Coord3D::new(0.0, 0.0, 0.0);
        let behavior = SupplyWarehouseDockUpdate::new((*data).clone(), 1, &pos);
        let mut module = SupplyWarehouseDockUpdateModule::new(
            behavior,
            &AsciiString::from("SupplyWarehouseDockUpdate"),
            data,
        );

        let dock = module
            .get_supply_warehouse_dock_interface()
            .expect("SupplyWarehouseDockUpdate should expose dock interface");
        assert_eq!(dock.boxes_stored(), 4);

        dock.set_cash_value(0);
        assert_eq!(module.behavior.get_boxes_stored(), 0);
    }
}
