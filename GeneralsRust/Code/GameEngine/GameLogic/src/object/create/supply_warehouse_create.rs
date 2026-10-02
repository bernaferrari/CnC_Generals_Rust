//! SupplyWarehouseCreate module - Registers supply warehouses on creation
//!
//! C++ Source: GameLogic/Object/Create/SupplyWarehouseCreate.cpp

use std::sync::Arc;

use crate::common::ObjectID;
use crate::object::create::CreateModule;
use crate::player::ThePlayerList;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{CreateInterface, Thing as ThingTrait};

#[derive(Debug)]
pub struct SupplyWarehouseCreate {
    base: CreateModule,
}

impl SupplyWarehouseCreate {
    pub fn new(thing: Arc<dyn ThingTrait>) -> Self {
        Self {
            base: CreateModule::new(thing),
        }
    }

    fn register_supply_warehouse(&self, object_id: ObjectID) {
        if object_id == 0 {
            return;
        }

        if let Ok(list_guard) = ThePlayerList().read() {
            for player_arc in list_guard.iter().rev() {
                let Ok(mut player_guard) = player_arc.write() else {
                    continue;
                };
                let Some(manager) = player_guard.get_resource_manager_mut() else {
                    continue;
                };
                manager.add_supply_warehouse(object_id);
            }
        }
    }
}

impl CreateInterface for SupplyWarehouseCreate {
    fn on_create(&self) {
        let object_id = self
            .base
            .get_thing()
            .as_object()
            .map(|obj| obj.get_object_id())
            .unwrap_or_default();
        self.register_supply_warehouse(object_id);
    }

    fn on_create_with_owner(&self, owner: &mut dyn std::any::Any) {
        // Object::init_object already holds its owner mutably. The Thing
        // handle would acquire the same object lock a second time.
        if let Some(object) = owner.downcast_ref::<crate::object::Object>() {
            self.register_supply_warehouse(object.get_id());
        }
    }

    fn on_build_complete(&self) {
        self.base.on_build_complete();
    }

    fn should_do_on_build_complete(&self) -> bool {
        self.base.should_do_on_build_complete()
    }
}

impl Snapshotable for SupplyWarehouseCreate {
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
