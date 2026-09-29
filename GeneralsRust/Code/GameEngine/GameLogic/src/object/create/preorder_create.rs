//! PreorderCreate module - Sets preorder model condition on build completion
//!
//! C++ Source: GameLogic/Object/Create/PreorderCreate.cpp

use std::sync::Arc;

use crate::common::ModelConditionFlags;

use crate::object::create::CreateModule;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{CreateInterface, Thing as ThingTrait};

#[derive(Debug)]
pub struct PreorderCreate {
    base: CreateModule,
}

impl PreorderCreate {
    pub fn new(thing: Arc<dyn ThingTrait>) -> Self {
        Self {
            base: CreateModule::new(thing),
        }
    }
}

impl CreateInterface for PreorderCreate {
    fn on_create(&self) {}

    fn on_build_complete(&self) {}

    fn on_build_complete_with_owner(&self, owner: &mut dyn std::any::Any) {
        let Some(obj) = owner.downcast_mut::<crate::object::Object>() else {
            return;
        };
        if obj.get_id() == 0 {
            return;
        }

        if let Some(player) = obj.get_controlling_player() {
            if let Ok(player_guard) = player.read() {
                if player_guard.did_player_preorder() {
                    obj.set_model_condition_state(ModelConditionFlags::PREORDER);
                } else {
                    let _ = obj.clear_model_condition_flags(ModelConditionFlags::PREORDER);
                }
            }
        }
    }

    fn should_do_on_build_complete(&self) -> bool {
        self.base.should_do_on_build_complete()
    }
}

impl Snapshotable for PreorderCreate {
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
