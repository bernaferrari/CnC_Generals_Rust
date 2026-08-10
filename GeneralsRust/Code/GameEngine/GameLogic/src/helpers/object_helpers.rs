// FiringTracker, ObjectHeldHelper, and ObjectDisabledHelper
//
// Split from `helpers.rs` for module-size parity.
// Observable behavior is unchanged.

/// Firing tracker for weapon firing statistics (matching C++ FiringTracker).
#[derive(Debug)]
pub struct FiringTracker {
    object_id: ObjectID,
    consecutive_shots: i32,
    victim_id: ObjectID,
    frame_to_start_cooldown: UnsignedInt,
    frame_to_force_reload: UnsignedInt,
    frame_to_stop_looping_sound: UnsignedInt,
    audio_handle: crate::common::audio::AudioHandle,
    last_shot_frame: UnsignedInt,
}

impl Drop for FiringTracker {
    fn drop(&mut self) {
        if self.audio_handle != 0 {
            if let Some(audio) = TheAudio::get() {
                audio.remove_audio_event(self.audio_handle);
            }
            self.audio_handle = 0;
        }
    }
}

impl FiringTracker {
    pub fn new(object_id: ObjectID) -> Self {
        if object_id != INVALID_ID {
            TheGameLogic::set_wake_frame(object_id, crate::modules::UPDATE_SLEEP_FOREVER);
        }

        Self {
            object_id,
            consecutive_shots: 0,
            victim_id: INVALID_ID,
            frame_to_start_cooldown: 0,
            frame_to_force_reload: 0,
            frame_to_stop_looping_sound: 0,
            audio_handle: 0,
            last_shot_frame: 0,
        }
    }

    pub fn get_last_shot_frame(&self) -> UnsignedInt {
        self.last_shot_frame
    }

    pub fn get_last_shot_victim(&self) -> ObjectID {
        self.victim_id
    }

    pub fn get_num_consecutive_shots_at_victim(&self, victim_id: ObjectID) -> i32 {
        if victim_id != INVALID_ID && victim_id == self.victim_id {
            self.consecutive_shots
        } else {
            0
        }
    }

    pub(crate) fn xfer_cpp_runtime_state(
        &mut self,
        xfer: &mut dyn game_engine::common::system::Xfer,
    ) -> Result<(), String> {
        xfer.xfer_int(&mut self.consecutive_shots)
            .map_err(|err| format!("FiringTracker xfer consecutive_shots: {err:?}"))?;
        xfer.xfer_object_id(&mut self.victim_id)
            .map_err(|err| format!("FiringTracker xfer victim_id: {err:?}"))?;
        xfer.xfer_unsigned_int(&mut self.frame_to_start_cooldown)
            .map_err(|err| format!("FiringTracker xfer frame_to_start_cooldown: {err:?}"))?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn set_xfer_test_state(
        &mut self,
        consecutive_shots: i32,
        victim_id: ObjectID,
        frame_to_start_cooldown: UnsignedInt,
        frame_to_force_reload: UnsignedInt,
        frame_to_stop_looping_sound: UnsignedInt,
    ) {
        self.consecutive_shots = consecutive_shots;
        self.victim_id = victim_id;
        self.frame_to_start_cooldown = frame_to_start_cooldown;
        self.frame_to_force_reload = frame_to_force_reload;
        self.frame_to_stop_looping_sound = frame_to_stop_looping_sound;
    }

    #[cfg(test)]
    pub(crate) fn xfer_test_state(&self) -> (i32, ObjectID, UnsignedInt, UnsignedInt, UnsignedInt) {
        (
            self.consecutive_shots,
            self.victim_id,
            self.frame_to_start_cooldown,
            self.frame_to_force_reload,
            self.frame_to_stop_looping_sound,
        )
    }

    pub fn shot_fired(&mut self, weapon: &crate::weapon::Weapon, victim_id: ObjectID) {
        let now = TheGameLogic::get_frame();
        self.last_shot_frame = now;

        let Some(owner_arc) = TheGameLogic::find_object_by_id(self.object_id) else {
            return;
        };

        let mut owner_guard = match owner_arc.write() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        let victim_has_faerie_fire = TheGameLogic::find_object_by_id(victim_id)
            .map(|victim| {
                victim
                    .read()
                    .ok()
                    .map(|victim_guard| {
                        victim_guard.test_status(crate::common::ObjectStatusTypes::FaerieFire)
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false);

        if victim_has_faerie_fire {
            if !owner_guard
                .get_weapon_bonus_condition()
                .contains(crate::common::types::WeaponBonusConditionFlags::TARGET_FAERIE_FIRE)
            {
                owner_guard.set_weapon_bonus_condition(
                    crate::common::types::WeaponBonusConditionType::TargetFaerieFire,
                );
            }
        } else if owner_guard
            .get_weapon_bonus_condition()
            .contains(crate::common::types::WeaponBonusConditionFlags::TARGET_FAERIE_FIRE)
        {
            owner_guard.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::TargetFaerieFire,
            );
        }

        if victim_id == self.victim_id {
            self.consecutive_shots += 1;
        } else if now < self.frame_to_start_cooldown {
            self.consecutive_shots += 1;
            self.victim_id = victim_id;
        } else {
            self.consecutive_shots = 1;
            self.victim_id = victim_id;
        }

        let template = weapon.get_template();
        if template.auto_reload_when_idle_frames > 0 {
            self.frame_to_force_reload = now.saturating_add(template.auto_reload_when_idle_frames);
        }

        if template.continuous_fire_coast_frames > 0 {
            self.frame_to_start_cooldown = weapon
                .get_possible_next_shot_frame()
                .saturating_add(template.continuous_fire_coast_frames);
        } else {
            self.frame_to_start_cooldown = 0;
        }

        let shots_needed_one = template.continuous_fire_one_shots_needed;
        let shots_needed_two = template.continuous_fire_two_shots_needed;

        let bonus_flags = owner_guard.get_weapon_bonus_condition();
        if bonus_flags
            .contains(crate::common::types::WeaponBonusConditionFlags::CONTINUOUS_FIRE_MEAN)
        {
            if self.consecutive_shots < shots_needed_one {
                self.cool_down(&mut owner_guard);
            } else if self.consecutive_shots > shots_needed_two {
                self.speed_up(&mut owner_guard);
            }
        } else if bonus_flags
            .contains(crate::common::types::WeaponBonusConditionFlags::CONTINUOUS_FIRE_FAST)
        {
            if self.consecutive_shots < shots_needed_two {
                self.cool_down(&mut owner_guard);
            }
        } else if self.consecutive_shots > shots_needed_one {
            self.speed_up(&mut owner_guard);
        }

        let fire_sound_loop_time = template.fire_sound_loop_time;
        if fire_sound_loop_time != 0 {
            let mut needs_restart = self.frame_to_stop_looping_sound == 0;
            if !needs_restart {
                if self.audio_handle == 0 {
                    needs_restart = true;
                } else {
                    let _manager =
                        get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
                    if self.audio_handle == 0 {
                        needs_restart = true;
                    }
                }
            }

            if needs_restart {
                let sound = template.fire_sound.clone();
                if !sound.is_empty() {
                    let mut event = AudioEventRts::new(sound.name().to_string());
                    event.set_object_id(self.object_id);
                    if let Some(audio) = TheAudio::get() {
                        self.audio_handle = audio.add_audio_event(&event);
                    }
                }
            }
            self.frame_to_stop_looping_sound =
                now.saturating_add(fire_sound_loop_time as UnsignedInt);
        } else {
            let sound = template.fire_sound.clone();
            if !sound.is_empty() {
                let mut event = AudioEventRts::new(sound.name().to_string());
                event.set_object_id(self.object_id);
                if let Some(audio) = TheAudio::get() {
                    audio.add_audio_event(&event);
                }
            }
            self.frame_to_stop_looping_sound = 0;
        }

        let sleep_time = self.calc_time_to_sleep(now);
        TheGameLogic::set_wake_frame(self.object_id, sleep_time);
    }

    pub fn update(&mut self) -> crate::modules::UpdateSleepTime {
        let now = TheGameLogic::get_frame();

        if self.frame_to_force_reload != 0 && now >= self.frame_to_force_reload {
            if let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) {
                if let Ok(mut guard) = owner.write() {
                    let _ = guard.reload_all_ammo(true);
                }
            }
            self.frame_to_force_reload = 0;
        }

        if self.frame_to_stop_looping_sound != 0 && now >= self.frame_to_stop_looping_sound {
            if let Some(audio) = TheAudio::get() {
                audio.remove_audio_event(self.audio_handle);
            }
            self.audio_handle = 0;
            self.frame_to_stop_looping_sound = 0;
        }

        if self.frame_to_start_cooldown != 0 && now > self.frame_to_start_cooldown {
            self.frame_to_start_cooldown =
                now.saturating_add(crate::common::LOGICFRAMES_PER_SECOND);
            if let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) {
                if let Ok(mut guard) = owner.write() {
                    self.cool_down(&mut guard);
                }
            }
            return crate::modules::UpdateSleepTime::Frames(crate::common::LOGICFRAMES_PER_SECOND);
        }

        self.calc_time_to_sleep(now)
    }

    fn calc_time_to_sleep(&self, now: UnsignedInt) -> crate::modules::UpdateSleepTime {
        if self.frame_to_stop_looping_sound == 0
            && self.frame_to_start_cooldown == 0
            && self.frame_to_force_reload == 0
        {
            return crate::modules::UpdateSleepTime::Forever;
        }

        let mut sleep_time = u32::MAX;

        if self.frame_to_stop_looping_sound != 0 {
            if self.frame_to_stop_looping_sound <= now {
                sleep_time = 0;
            } else {
                sleep_time = sleep_time.min(self.frame_to_stop_looping_sound - now);
            }
        }

        if self.frame_to_start_cooldown != 0 {
            if self.frame_to_start_cooldown <= now {
                sleep_time = 0;
            } else {
                sleep_time = sleep_time.min(self.frame_to_start_cooldown - now);
            }
        }

        if self.frame_to_force_reload != 0 {
            if self.frame_to_force_reload <= now {
                sleep_time = 0;
            } else {
                sleep_time = sleep_time.min(self.frame_to_force_reload - now);
            }
        }

        crate::modules::UpdateSleepTime::from_u32(sleep_time)
    }

    fn speed_up(&mut self, owner: &mut Object) {
        let clear = crate::common::ModelConditionFlags::empty();
        let set = crate::common::ModelConditionFlags::empty();

        if owner
            .get_weapon_bonus_condition()
            .contains(crate::common::types::WeaponBonusConditionFlags::CONTINUOUS_FIRE_FAST)
        {
            // Already at max speed, nothing to do.
        } else if owner
            .get_weapon_bonus_condition()
            .contains(crate::common::types::WeaponBonusConditionFlags::CONTINUOUS_FIRE_MEAN)
        {
            if let Some(mut sound) = owner.get_template().get_per_unit_sound("VoiceRapidFire") {
                sound.set_object_id(self.object_id);
                if let Some(audio) = TheAudio::get() {
                    audio.add_audio_event(&sound);
                }
            }

            owner.set_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::ContinuousFireFast,
            );
            owner.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::ContinuousFireMean,
            );
        } else {
            owner.set_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::ContinuousFireMean,
            );
            owner.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::ContinuousFireFast,
            );
        }

        let _ = owner.clear_and_set_model_condition_flags(clear, set);
    }

    fn cool_down(&mut self, owner: &mut Object) {
        let clear = crate::common::ModelConditionFlags::empty();
        let set = crate::common::ModelConditionFlags::empty();

        let bonus_flags = owner.get_weapon_bonus_condition();
        if bonus_flags
            .contains(crate::common::types::WeaponBonusConditionFlags::CONTINUOUS_FIRE_FAST)
            || bonus_flags
                .contains(crate::common::types::WeaponBonusConditionFlags::CONTINUOUS_FIRE_MEAN)
        {
            owner.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::ContinuousFireFast,
            );
            owner.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::ContinuousFireMean,
            );
        } else {
            owner.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::ContinuousFireFast,
            );
            owner.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::ContinuousFireMean,
            );
            self.frame_to_start_cooldown = 0;
        }

        let _ = owner.clear_and_set_model_condition_flags(clear, set);

        self.consecutive_shots = 0;
        self.victim_id = INVALID_ID;
    }
}

/// Object held helper (matching C++ ObjectHeldHelper)
#[derive(Debug)]
pub struct ObjectHeldHelper {
    is_held: bool,
    holder_id: ObjectID,
}

impl ObjectHeldHelper {
    pub fn new() -> Self {
        Self {
            is_held: false,
            holder_id: INVALID_ID,
        }
    }

    pub fn is_held(&self) -> bool {
        self.is_held
    }

    pub fn set_held(&mut self, held: bool, holder_id: ObjectID) {
        self.is_held = held;
        self.holder_id = if held { holder_id } else { INVALID_ID };
    }
}

/// Object disabled helper (matching C++ ObjectDisabledHelper)
#[derive(Debug)]
#[allow(dead_code)]
pub struct ObjectDisabledHelper {
    disabled_mask: DisabledMaskType,
    disabled_until: [UnsignedInt; DISABLED_COUNT],
}

impl ObjectDisabledHelper {
    pub fn new() -> Self {
        Self {
            disabled_mask: DisabledMaskType::none(),
            disabled_until: [NEVER; DISABLED_COUNT],
        }
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled_mask.any()
    }

    pub fn set_disabled(&mut self, disabled_type: DisabledType, _until_frame: UnsignedInt) {
        self.disabled_mask.set_disabled(disabled_type);
        // Set the frame when this disability expires
    }

    pub fn clear_disabled(&mut self, disabled_type: DisabledType) {
        self.disabled_mask.clear(disabled_type);
    }
}
