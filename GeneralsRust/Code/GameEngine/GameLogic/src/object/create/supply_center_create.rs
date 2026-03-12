//! SupplyCenterCreate module - Registers supply centers on build completion
//!
//! C++ Source: GameLogic/Object/Create/SupplyCenterCreate.cpp

use std::sync::Arc;

use crate::object::create::CreateModule;
use crate::player::ThePlayerList;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{CreateInterface, Thing as ThingTrait};

#[derive(Debug)]
pub struct SupplyCenterCreate {
    base: CreateModule,
}

impl SupplyCenterCreate {
    pub fn new(thing: Arc<dyn ThingTrait>) -> Self {
        Self {
            base: CreateModule::new(thing),
        }
    }
}

impl CreateInterface for SupplyCenterCreate {
    fn on_create(&self) {}

    fn on_build_complete(&self) {
        if !self.base.should_do_on_build_complete() {
            return;
        }

        self.base.on_build_complete();

        let object_id = self
            .base
            .get_thing()
            .as_object()
            .map(|obj| obj.get_object_id())
            .unwrap_or_default();
        if object_id == 0 {
            return;
        }

        if let Ok(list_guard) = ThePlayerList().read() {
            for player_arc in list_guard.iter() {
                let Ok(mut player_guard) = player_arc.write() else {
                    continue;
                };
                let Some(manager) = player_guard.get_resource_manager_mut() else {
                    continue;
                };
                manager.add_supply_center(object_id);
            }
        }
    }

    fn should_do_on_build_complete(&self) -> bool {
        self.base.should_do_on_build_complete()
    }
}

impl Snapshotable for SupplyCenterCreate {
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
