//! StealthUpdate.cpp:846–912 on the exact installed owner. Construction stages
//! state; explicit admission supplies its birth clock and cosmetic stream.
use super::*;

impl StealthUpdateModule {
    pub(crate) fn new_staged(
        key: NameKeyType,
        data: Arc<StealthUpdateModuleData>,
        id: ObjectID,
    ) -> Self {
        Self {
            module_name_key: key,
            controller: Arc::new(Mutex::new(StealthController::new_at(
                Arc::clone(&data),
                id,
                0,
                0.0,
                false,
            ))),
            data,
            object_id: id,
            next_call_frame_and_phase: 0,
        }
    }
    pub(crate) fn bind_construction_owner(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        unpublished: bool,
    ) -> StealthUpdateHandle {
        let mut controller = self
            .controller
            .lock()
            .expect("new stealth controller poisoned");
        controller.owner = Some(Arc::downgrade(owner));
        if !unpublished && !controller.native_ready {
            let frame = TheGameLogic::get_frame();
            controller.initialize_at(frame, game_client_random_value_real(0.0, PI));
            self.next_call_frame_and_phase = if self.data.granted_by_special_power {
                crate::modules::UpdateSleepTime::Forever.to_u32()
            } else {
                frame.wrapping_add(1)
            };
        }
        Arc::clone(&self.controller)
    }
    pub(crate) fn admit_native(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        frame: u32,
        pulse: f32,
    ) -> Result<(bool, u32), String> {
        let mut controller = self
            .controller
            .lock()
            .map_err(|_| "stealth admission controller poisoned")?;
        controller.validate_native_owner(owner, false)?;
        if controller.native_ready {
            return Err("stealth already admitted or restored".into());
        }
        controller.initialize_at(frame, pulse);
        self.next_call_frame_and_phase = if self.data.granted_by_special_power {
            crate::modules::UpdateSleepTime::Forever.to_u32()
        } else {
            frame.wrapping_add(1)
        };
        Ok((self.data.innate_stealth, self.next_call_frame_and_phase))
    }
}
impl StealthController {
    fn initialize_at(&mut self, frame: u32, pulse: f32) {
        self.stealth_allowed_frame = frame.wrapping_add(self.data.stealth_delay_frames);
        self.pulse_phase = pulse;
        self.native_ready = true;
    }
    pub(crate) fn validate_native_owner(
        &self,
        owner: &Arc<RwLock<Object>>,
        require_ready: bool,
    ) -> Result<(), String> {
        let exact = self
            .owner
            .as_ref()
            .and_then(Weak::upgrade)
            .ok_or("stealth construction owner retired or unbound")?;
        if !Arc::ptr_eq(&exact, owner) {
            return Err("stealth belongs to another factory owner".into());
        }
        if require_ready && !self.native_ready {
            return Err("stealth needs explicit birth admission".into());
        }
        Ok(())
    }
    pub(crate) fn uses_rider_stealth(&self) -> bool {
        self.data.use_rider_stealth
    }
    pub(crate) fn birth_deadline(&self) -> u32 {
        self.stealth_allowed_frame
    }
    pub(crate) fn is_native_ready(&self) -> bool {
        self.native_ready
    }
    pub(crate) fn detection_orders_idle_enemies(&self) -> bool {
        self.data.order_idle_enemies_to_attack
    }
    pub(crate) fn mark_as_detected_at_owned(&mut self, frame: u32, frames: u32) -> bool {
        assert!(self.native_ready, "stealth must be admitted before reveal");
        let disguise = self.is_disguised();
        // CPP disguiseAsObject(NULL) changes the reveal transition only once
        // the visual disguise is installed. A pending template alone is not it.
        if disguise && self.disguised {
            self.disguise_as_template_name = None;
            self.disguise_as_player_index = 0;
            self.disguise_transition_frames = self.data.disguise_reveal_transition_frames;
            self.transitioning_to_disguise = false;
            self.disguise_halfpoint_reached = false;
        }
        if frames == 0 {
            self.detection_expires_frame = frame.wrapping_add(self.data.stealth_delay_frames);
        } else if self.detection_expires_frame < frame.wrapping_add(frames) {
            self.detection_expires_frame = frame.wrapping_add(frames);
        }
        // CPP markAsDetected changes the deadline; update observes it later.
        // It does not set DETECTED/STEALTHED immediately as the old adapter does.
        disguise
    }
    pub(crate) fn detection_deadline(&self) -> u32 {
        self.detection_expires_frame
    }
}

#[cfg(test)]
impl StealthUpdateModuleData {
    pub(crate) fn from_owned_test_fields(fields: &[(&str, &[&str])]) -> Self {
        let mut data = Self::default();
        for (token, values) in fields {
            let field = STEALTH_UPDATE_FIELDS
                .iter()
                .find(|field| field.token == *token)
                .unwrap();
            (field.parse)(&mut INI::new(), &mut data, values).unwrap();
        }
        data
    }
}
