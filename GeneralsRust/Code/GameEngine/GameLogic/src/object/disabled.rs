//! Object disable-enter / invulnerable helpers (C++ Object.cpp).
//!
//! `set_disabled_until` enter-path, dozer cancel on disable-edge, and
//! `go_invulnerable` live here so `object_upgrade.rs` stays a dispatcher.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;
use crate::object::behavior::auto_heal_behavior::AutoHealBehavior;
use crate::object::update::ai_update::dozer_ai_update::DozerTask;

impl Object {
    /// C++ `Object::setDisabledUntil` (Object.cpp:2050-2187).
    pub fn set_disabled_until(&mut self, disabled_type: DisabledType, frame: UnsignedInt) {
        let edge_case = !self.is_disabled();

        if disabled_type == DisabledType::DisabledUnmanned && !self.is_kind_of(KindOf::Drone) {
            self.play_misc_audio_at_position(|misc| {
                misc.splatter_vehicle_pilots_brain
                    .playable_event_name()
                    .to_string()
            });
        } else if matches!(
            disabled_type,
            DisabledType::DisabledUnderpowered
                | DisabledType::DisabledEmp
                | DisabledType::DisabledSubdued
                | DisabledType::DisabledHacked
        ) {
            let already_power_disabled = [
                DisabledType::DisabledUnderpowered,
                DisabledType::DisabledEmp,
                DisabledType::DisabledSubdued,
                DisabledType::DisabledHacked,
            ]
            .into_iter()
            .any(|other| self.is_disabled_by_type(other));
            if !already_power_disabled {
                if self.is_kind_of(KindOf::Structure) {
                    self.play_misc_audio_at_position(|misc| {
                        misc.building_disabled.playable_event_name().to_string()
                    });
                } else if self.is_kind_of(KindOf::Vehicle) {
                    self.play_misc_audio_at_position(|misc| {
                        misc.vehicle_disabled.playable_event_name().to_string()
                    });
                }
            }
        }

        let type_index = self.get_disabled_type_index(disabled_type);
        let frame_changed = type_index
            .map(|index| self.disabled_till_frame[index] != frame)
            .unwrap_or(true);

        if frame_changed {
            if disabled_type != DisabledType::Held && !self.is_disabled_by_type(disabled_type) {
                self.pause_all_special_powers(true);
            }

            if let Some(index) = type_index {
                self.disabled_till_frame[index] = frame;
            }
            let now = crate::helpers::TheGameLogic::get_frame();
            if frame > now {
                self.disabled_mask.set_disabled(disabled_type);
            } else {
                self.disabled_mask.clear(disabled_type);
            }

            if self.is_disabled() {
                if !matches!(
                    disabled_type,
                    DisabledType::Held
                        | DisabledType::DisabledScriptDisabled
                        | DisabledType::DisabledUnmanned
                ) {
                    if let Some(drawable) = &self.drawable {
                        if let Ok(mut draw_guard) = drawable.write() {
                            draw_guard
                                .set_tint_status(crate::object::drawable::TintStatus::DISABLED);
                        }
                    }
                }
            }

            if let Some(contain) = &self.contain {
                if let Ok(contain_guard) = contain.lock() {
                    if let Some(rider_id) = contain_guard.get_rider_id() {
                        if let Some(rider) =
                            crate::helpers::TheGameLogic::find_object_by_id(rider_id)
                        {
                            if let Ok(mut rider_guard) = rider.write() {
                                rider_guard.set_disabled_until(disabled_type, frame);
                            }
                        }
                    }
                }
            }

            if self.is_kind_of(KindOf::SpawnsAreTheWeapons) {
                self.order_spawn_slaves_disabled_until(disabled_type, frame);
            }
        }

        if disabled_type == DisabledType::DisabledUnmanned && !self.is_kind_of(KindOf::Drone) {
            self.handle_unmanned_disable_side_effects();
        }

        if edge_case && self.is_disabled() {
            self.on_disabled_edge(true);
        }
    }

    fn play_misc_audio_at_position(
        &self,
        pick: impl FnOnce(&game_engine::common::ini::ini_misc_audio::MiscAudio) -> String,
    ) {
        let Some(audio) = crate::helpers::TheAudio::get() else {
            return;
        };
        let Some(misc_audio) = game_engine::common::ini::ini_misc_audio::get_misc_audio() else {
            return;
        };
        let misc_audio = misc_audio.read();
        let sound_name = pick(&misc_audio);
        if sound_name.is_empty() {
            return;
        }
        let mut event = crate::object::special_power_template::AudioEventRts::new(sound_name);
        let pos = self.get_position();
        event.set_position(&(pos.x, pos.y, pos.z));
        audio.add_audio_event(&event);
    }

    fn order_spawn_slaves_disabled_until(
        &mut self,
        disabled_type: DisabledType,
        frame: UnsignedInt,
    ) {
        for behavior in &self.behaviors {
            if let Ok(mut guard) = behavior.lock() {
                if let Some(spawn) = guard.get_spawn_behavior_interface() {
                    let _ = spawn.order_slaves_disabled_until(disabled_type, frame);
                    return;
                }
            }
        }
    }

    fn handle_unmanned_disable_side_effects(&mut self) {
        if self.test_status(ObjectStatusTypes::IsCarBomb) {
            let sniper_id = self
                .body
                .as_ref()
                .and_then(|body| body.lock().ok())
                .and_then(|body_guard| {
                    body_guard
                        .get_last_damage_info()
                        .map(|info| info.input.source_id)
                });
            if let Some(sniper_id) = sniper_id {
                if sniper_id != INVALID_ID {
                    if let Some(sniper) = crate::helpers::TheGameLogic::find_object_by_id(sniper_id)
                    {
                        if let Ok(mut sniper_guard) = sniper.write() {
                            sniper_guard.score_the_kill(self);
                        }
                    }
                }
            }
            self.kill(None, None);
            return;
        }

        if let Some(tracker) = &self.experience_tracker {
            if let Ok(mut guard) = tracker.lock() {
                let _ = guard.set_experience_and_level(0, &[]);
            }
        }
        self.with_friend_module_by_name::<AutoHealBehavior, _, _>("AutoHealBehavior", |heal| {
            heal.undo_upgrade();
        });
    }

    /// C++ `Object::onDisabledEdge` dozer cancel (Object.cpp:3796-3802).
    pub(super) fn cancel_dozer_task_on_disabled_edge(&mut self, becoming_disabled: bool) {
        if !becoming_disabled {
            return;
        }
        let Some(ai) = self.ai.clone() else {
            return;
        };
        let Ok(mut ai_guard) = ai.lock() else {
            return;
        };
        let Some(dozer_ai) = ai_guard.get_dozer_ai_update_interface_mut() else {
            return;
        };
        let task = dozer_ai.get_current_task();
        if task != DozerTask::Invalid {
            dozer_ai.cancel_task(task);
        }
    }

    /// C++ `Object::goInvulnerable` (Object.cpp:6225-6233). `time` is already frames.
    pub fn go_invulnerable(&mut self, time: UnsignedInt) {
        self.friend_set_undetected_defector(time > 0);
        self.invulnerable_until_frame = 0;
        if let Some(helper) = &self.defection_helper {
            if let Ok(mut guard) = helper.lock() {
                let now = crate::helpers::TheGameLogic::get_frame();
                guard.start_defection_timer(time, false, now, self.is_undetected_defector());
            }
        }
    }
}
