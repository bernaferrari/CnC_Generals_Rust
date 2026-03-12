//! SpecialPowerCreate module - Starts special power cooldowns on build completion
//!
//! C++ Source: GameLogic/Object/Create/SpecialPowerCreate.cpp

use std::sync::Arc;

use crate::helpers::TheGameLogic;
use crate::object::create::CreateModule;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{CreateInterface, Thing as ThingTrait};

#[derive(Debug)]
pub struct SpecialPowerCreate {
    base: CreateModule,
}

impl SpecialPowerCreate {
    pub fn new(thing: Arc<dyn ThingTrait>) -> Self {
        Self {
            base: CreateModule::new(thing),
        }
    }
}

impl CreateInterface for SpecialPowerCreate {
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

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return;
        };
        let Ok(object_guard) = object_arc.read() else {
            return;
        };

        for behavior_arc in &object_guard.behaviors {
            if let Ok(mut behavior_guard) = behavior_arc.lock() {
                if let Some(sp) = behavior_guard.get_special_power() {
                    sp.on_special_power_creation();
                }
            }
        }
    }

    fn should_do_on_build_complete(&self) -> bool {
        self.base.should_do_on_build_complete()
    }
}

impl Snapshotable for SpecialPowerCreate {
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
