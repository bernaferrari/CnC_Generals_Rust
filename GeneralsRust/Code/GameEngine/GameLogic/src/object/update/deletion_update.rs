// DeletionUpdate - Counts down a lifetime and destroys object when it reaches zero
// Author: Graham Smallwood, August 2002
// Ported to Rust

use crate::prelude::*;

#[derive(Debug, Clone)]
pub struct DeletionUpdateModuleData {
    pub min_frames: u32,
    pub max_frames: u32,
}

impl Default for DeletionUpdateModuleData {
    fn default() -> Self {
        Self {
            min_frames: 0,
            max_frames: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeletionUpdate {
    thing: ThingId,
    #[allow(dead_code)]
    module_data: DeletionUpdateModuleData,
    die_frame: u32,
}

impl DeletionUpdate {
    pub fn new(thing: ThingId, module_data: DeletionUpdateModuleData) -> Self {
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        let delay = Self::calc_sleep_delay(&module_data, current_frame);
        Self {
            thing,
            module_data,
            die_frame: delay,
        }
    }

    fn calc_sleep_delay(data: &DeletionUpdateModuleData, current_frame: u32) -> u32 {
        let mut delay = game_logic_random_value(data.min_frames, data.max_frames);
        if delay < 1 {
            delay = 1;
        }
        current_frame + delay
    }

    pub fn set_lifetime_range(&mut self, min_frames: u32, max_frames: u32, current_frame: u32) {
        self.die_frame = Self::calc_sleep_delay(
            &DeletionUpdateModuleData {
                min_frames,
                max_frames,
            },
            current_frame,
        );
    }

    pub fn update(&mut self, ctx: &mut UpdateContext<'_>) -> UpdateSleepTime {
        // C++ destroys whenever the scheduled update is invoked; timing is owned by the scheduler.
        if let Some(object) = ctx.game_logic.find_object(self.thing) {
            ctx.game_logic.destroy_object(object.id());
        }
        UpdateSleepTime::Forever
    }

    pub fn save(&self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("DeletionUpdate::save failed to xfer {field}: {err}");
            }
        };

        xfer.xfer_version_write(1);
        let mut die_frame = self.die_frame;
        xfer_io(xfer.xfer_u32(&mut die_frame), "die_frame");
    }

    pub fn load(&mut self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("DeletionUpdate::load failed to xfer {field}: {err}");
            }
        };

        let version = xfer.xfer_version_read();
        if version >= 1 {
            xfer_io(xfer.xfer_u32(&mut self.die_frame), "die_frame");
        }
    }
}

fn game_logic_random_value(min: u32, max: u32) -> u32 {
    if min >= max {
        return min;
    }
    crate::helpers::game_logic_random_value(min, max)
}
