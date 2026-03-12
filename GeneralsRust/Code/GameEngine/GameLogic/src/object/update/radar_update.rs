// radar_update.rs
// RadarUpdate - Updating a radar on an object
// FILE: RadarUpdate.rs (ported from RadarUpdate.cpp/.h)
// Author: Colin Day, April 2002
// Ported to Rust
//

use crate::common::xfer::XferExt;
use crate::prelude::*;
use game_engine::common::system::{Snapshotable, Xfer};

#[derive(Debug, Clone)]
pub struct RadarUpdateModuleData {
    pub radar_extending_frames: u32,
}

impl Default for RadarUpdateModuleData {
    fn default() -> Self {
        Self {
            radar_extending_frames: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadarStatus {
    Idle,
    Extending,
    Active,
}

#[derive(Debug, Clone)]
pub struct RadarUpdate {
    thing: ThingId,
    module_data: RadarUpdateModuleData,
    status: RadarStatus,
    next_ready_frame: u32,
}

impl RadarUpdate {
    pub fn new(thing: ThingId, module_data: RadarUpdateModuleData) -> Self {
        Self {
            thing,
            module_data,
            status: RadarStatus::Idle,
            next_ready_frame: 0,
        }
    }

    pub fn is_active(&self) -> bool {
        self.status == RadarStatus::Active
    }

    pub fn extend_radar(&mut self, ctx: &mut UpdateContext<'_>) {
        if self.status != RadarStatus::Idle {
            return;
        }

        let now = ctx.game_logic.get_frame();
        self.status = RadarStatus::Extending;
        self.next_ready_frame = now + self.module_data.radar_extending_frames;

        if let Some(object) = ctx.game_logic.find_object_mut(self.thing) {
            object.set_model_condition_state(ModelConditionFlag::RadarExtending);
        }
    }

    pub fn update(&mut self, ctx: &mut UpdateContext<'_>) -> UpdateSleepTime {
        let now = ctx.game_logic.get_frame();

        if self.status == RadarStatus::Extending && now >= self.next_ready_frame {
            self.status = RadarStatus::Active;
            if let Some(object) = ctx.game_logic.find_object_mut(self.thing) {
                object.clear_model_condition_state(ModelConditionFlag::RadarExtending);
                object.set_model_condition_state(ModelConditionFlag::RadarUpgraded);
            }
        }

        UpdateSleepTime::None
    }

    pub fn crc(&self, _xfer: &mut dyn Xfer) {
        // Implementation for CRC check
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("RadarUpdate::xfer failed to xfer {field}: {err}");
            }
        };

        xfer.xfer_version_write(1);
        let mut status = self.status as u32;
        xfer_io(xfer.xfer_u32(&mut status), "status");
        xfer_io(
            xfer.xfer_u32(&mut self.next_ready_frame),
            "next_ready_frame",
        );
    }

    pub fn load(&mut self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("RadarUpdate::load failed to xfer {field}: {err}");
            }
        };

        let version = xfer.xfer_version_read();
        if version >= 1 {
            let mut status_val = 0u32;
            xfer_io(xfer.xfer_u32(&mut status_val), "status");
            self.status = match status_val {
                0 => RadarStatus::Idle,
                1 => RadarStatus::Extending,
                2 => RadarStatus::Active,
                _ => RadarStatus::Idle,
            };
            xfer_io(
                xfer.xfer_u32(&mut self.next_ready_frame),
                "next_ready_frame",
            );
        }
    }
}
