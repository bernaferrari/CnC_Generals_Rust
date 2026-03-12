// FlammableUpdate - Manages Aflame and Burned statuses and their effects
// Author: Graham Smallwood, April 2002
// Ported to Rust

use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::prelude::*;

#[derive(Debug, Clone)]
pub struct FlammableUpdateModuleData {
    pub burned_delay: u32,
    pub aflame_duration: u32,
    pub aflame_damage_delay: u32,
    pub aflame_damage_amount: i32,
    pub burning_sound_name: String,
    pub flame_damage_limit_data: f32,
    pub flame_damage_expiration_delay: u32,
}

impl Default for FlammableUpdateModuleData {
    fn default() -> Self {
        Self {
            burned_delay: 0,
            aflame_duration: 0,
            aflame_damage_delay: 0,
            aflame_damage_amount: 0,
            burning_sound_name: String::new(),
            flame_damage_limit_data: 20.0,
            flame_damage_expiration_delay: LOGICFRAMES_PER_SECOND * 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlammabilityStatusType {
    Normal,
    Aflame,
    Burned,
}

#[derive(Debug, Clone)]
pub struct FlammableUpdate {
    thing: ThingId,
    module_data: FlammableUpdateModuleData,
    status: FlammabilityStatusType,
    aflame_end_frame: u32,
    burned_end_frame: u32,
    damage_end_frame: u32,
    audio_handle: Option<AudioHandle>,
    flame_damage_limit: f32,
    last_flame_damage_dealt: u32,
}

impl FlammableUpdate {
    pub fn new(thing: ThingId, module_data: FlammableUpdateModuleData) -> Self {
        Self {
            thing,
            flame_damage_limit: module_data.flame_damage_limit_data,
            module_data,
            status: FlammabilityStatusType::Normal,
            aflame_end_frame: 0,
            burned_end_frame: 0,
            damage_end_frame: 0,
            audio_handle: None,
            last_flame_damage_dealt: 0,
        }
    }

    pub fn on_damage(&mut self, damage_info: &DamageInfo, ctx: &mut UpdateContext<'_>) {
        if matches!(
            damage_info.input.damage_type,
            DamageType::Flame | DamageType::ParticleBeam
        ) {
            let now = ctx.game_logic.get_frame();

            // If it has been a long time since our last flame damage, reset the threshold
            if now - self.module_data.flame_damage_expiration_delay > self.last_flame_damage_dealt {
                self.flame_damage_limit = self.module_data.flame_damage_limit_data;
            }
            self.last_flame_damage_dealt = now;

            if let Some(object) = ctx.game_logic.find_object(self.thing) {
                let aflame = object.test_status(ObjectStatus::Aflame);
                let burned = object.test_status(ObjectStatus::Burned);

                // If I'm not on fire and I haven't burned up, see if I should try to catch fire
                if !aflame && !burned {
                    self.flame_damage_limit -= damage_info.output.actual_damage_dealt;
                    if self.flame_damage_limit <= 0.0 {
                        self.try_to_ignite(ctx);
                    }
                }
            }
        }
    }

    pub fn update(&mut self, ctx: &mut UpdateContext<'_>) -> UpdateSleepTime {
        debug_assert!(self.status == FlammabilityStatusType::Aflame);

        let now = ctx.game_logic.get_frame();

        if self.damage_end_frame != 0 && now >= self.damage_end_frame {
            self.damage_end_frame = now + self.module_data.aflame_damage_delay;
            self.do_aflame_damage(ctx);
        }

        if self.burned_end_frame != 0 && now >= self.burned_end_frame {
            // Set burned status but still aflame on independent timer
            if let Some(object) = ctx.game_logic.find_object_mut(self.thing) {
                object.set_status(ObjectStatusTypes::Burned.into(), true);
                object.set_model_condition_state(ModelConditionFlag::Smoldering);
            }
        }

        if self.aflame_end_frame != 0 && now >= self.aflame_end_frame {
            // Determine new status
            let is_burned = ctx
                .game_logic
                .find_object(self.thing)
                .map(|o| o.test_status(ObjectStatus::Burned))
                .unwrap_or(false);

            if is_burned {
                self.status = FlammabilityStatusType::Burned;
            } else {
                self.status = FlammabilityStatusType::Normal;
            }

            self.stop_burning_sound(ctx);

            // No longer on fire - clear status and model condition
            if let Some(object) = ctx.game_logic.find_object_mut(self.thing) {
                object.clear_status(ObjectStatusTypes::Aflame.into());

                if let Some(body) = object.get_body_module() {
                    body.set_aflame(false);
                }

                object.clear_model_condition_state(ModelConditionFlag::Aflame);
            }
        }

        self.calc_sleep_time(now)
    }

    fn calc_sleep_time(&self, now: u32) -> UpdateSleepTime {
        if self.status == FlammabilityStatusType::Aflame
            && self.aflame_end_frame != 0
            && self.aflame_end_frame > now
        {
            let mut soonest = self.aflame_end_frame;

            if self.burned_end_frame != 0
                && self.burned_end_frame < soonest
                && self.burned_end_frame > now
            {
                soonest = self.burned_end_frame;
            }

            if self.damage_end_frame != 0
                && self.damage_end_frame < soonest
                && self.damage_end_frame > now
            {
                soonest = self.damage_end_frame;
            }

            debug_assert!(soonest > now);
            UpdateSleepTime::Frames(soonest - now)
        } else {
            UpdateSleepTime::Forever
        }
    }

    pub fn try_to_ignite(&mut self, ctx: &mut UpdateContext<'_>) {
        if self.status != FlammabilityStatusType::Normal {
            return;
        }

        // Get frame now before mutable borrow
        let now = ctx.game_logic.get_frame();

        let Some(object) = ctx.game_logic.find_object_mut(self.thing) else {
            return;
        };

        object.set_status(ObjectStatus::Aflame.into(), true);

        if let Some(body) = object.get_body_module() {
            body.set_aflame(true);
        }

        object.set_model_condition_state(ModelConditionFlag::Aflame);

        // Check for FireSpreadUpdate, then drop object borrow before using ctx again.
        let fire_spread = object.find_update_module("FireSpreadUpdate");

        if let Some(fire_spread) = fire_spread {
            let started =
                fire_spread.with_module_downcast::<
                    crate::object::update::fire_spread_update::FireSpreadUpdateModule,
                    _,
                    _,
                >(|module| {
                    module.behavior_mut().start_fire_spreading(ctx);
                });
            if started.is_none() {
                log::debug!(
                    "FlammableUpdate::try_to_ignite missing FireSpreadUpdateModule downcast for {:?}",
                    self.thing
                );
            }
        }

        self.start_burning_sound(ctx);

        self.status = FlammabilityStatusType::Aflame;

        self.aflame_end_frame = now + self.module_data.aflame_duration;
        self.burned_end_frame = if self.module_data.burned_delay > 0 {
            now + self.module_data.burned_delay
        } else {
            0
        };
        self.damage_end_frame = if self.module_data.aflame_damage_delay > 0 {
            now + self.module_data.aflame_damage_delay
        } else {
            0
        };
    }

    fn do_aflame_damage(&self, ctx: &mut UpdateContext<'_>) {
        let Some(object) = ctx.game_logic.find_object_mut(self.thing) else {
            return;
        };

        let mut damage_info = DamageInfo::with_simple(
            self.module_data.aflame_damage_amount as f32,
            self.thing,
            DamageType::Flame,
            DeathType::Burned,
        );
        if let Err(err) = object.attempt_damage(&mut damage_info) {
            log::debug!(
                "FlammableUpdate::do_aflame_damage failed for {:?}: {}",
                self.thing,
                err
            );
        }
    }

    fn start_burning_sound(&mut self, ctx: &mut UpdateContext<'_>) {
        if !self.module_data.burning_sound_name.is_empty() {
            let audio_event =
                AudioEventRTS::from_sound_file(self.module_data.burning_sound_name.clone());
            if let Some(audio) = ctx.audio.as_mut() {
                self.audio_handle = Some(audio.add_audio_event(&audio_event));
            }
        }
    }

    fn stop_burning_sound(&mut self, ctx: &mut UpdateContext<'_>) {
        if let Some(handle) = self.audio_handle.take() {
            if let Some(audio) = ctx.audio.as_mut() {
                audio.remove_audio_event(handle);
            }
        }
    }

    pub fn would_ignite(&self) -> bool {
        self.status == FlammabilityStatusType::Normal
    }

    pub fn save(&self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("FlammableUpdate::save failed to xfer {field}: {err}");
            }
        };

        xfer.xfer_version_write(1);
        let mut status = self.status as u32;
        xfer_io(xfer.xfer_u32(&mut status), "status");
        let mut aflame_end_frame = self.aflame_end_frame;
        xfer_io(xfer.xfer_u32(&mut aflame_end_frame), "aflame_end_frame");
        let mut burned_end_frame = self.burned_end_frame;
        xfer_io(xfer.xfer_u32(&mut burned_end_frame), "burned_end_frame");
        let mut damage_end_frame = self.damage_end_frame;
        xfer_io(xfer.xfer_u32(&mut damage_end_frame), "damage_end_frame");
        let mut flame_damage_limit = self.flame_damage_limit;
        xfer_io(xfer.xfer_f32(&mut flame_damage_limit), "flame_damage_limit");
        let mut last_flame_damage_dealt = self.last_flame_damage_dealt;
        xfer_io(
            xfer.xfer_u32(&mut last_flame_damage_dealt),
            "last_flame_damage_dealt",
        );
    }

    pub fn load(&mut self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("FlammableUpdate::load failed to xfer {field}: {err}");
            }
        };

        let version = xfer.xfer_version_read();
        if version >= 1 {
            let mut status_val = 0u32;
            xfer_io(xfer.xfer_u32(&mut status_val), "status");
            self.status = match status_val {
                0 => FlammabilityStatusType::Normal,
                1 => FlammabilityStatusType::Aflame,
                2 => FlammabilityStatusType::Burned,
                _ => FlammabilityStatusType::Normal,
            };
            xfer_io(
                xfer.xfer_u32(&mut self.aflame_end_frame),
                "aflame_end_frame",
            );
            xfer_io(
                xfer.xfer_u32(&mut self.burned_end_frame),
                "burned_end_frame",
            );
            xfer_io(
                xfer.xfer_u32(&mut self.damage_end_frame),
                "damage_end_frame",
            );
            xfer_io(
                xfer.xfer_f32(&mut self.flame_damage_limit),
                "flame_damage_limit",
            );
            xfer_io(
                xfer.xfer_u32(&mut self.last_flame_damage_dealt),
                "last_flame_damage_dealt",
            );
        }
    }
}

const LOGICFRAMES_PER_SECOND: u32 = 30;
