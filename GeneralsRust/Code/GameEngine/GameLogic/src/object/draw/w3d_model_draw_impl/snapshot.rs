impl Snapshotable for W3DModelDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let _ = xfer;
        // C++ W3DModelDraw::crc only extends DrawModule::crc; it does not hash
        // the versioned W3DModelDraw::xfer payload.
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ parity: W3DModelDraw::xfer (version 2) serializes recoil vectors,
        // sub-object visibility list, and optional animation frame payload.
        const CURRENT_VERSION: XferVersion = 2;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        for slot in 0..WEAPONSLOT_COUNT {
            let mut recoil_info_count = self
                .weapon_recoil_info
                .get(slot)
                .map(|entries| entries.len())
                .unwrap_or_default()
                .min(u8::MAX as usize) as u8;
            xfer.xfer_unsigned_byte(&mut recoil_info_count)
                .map_err(|e| e.to_string())?;

            if xfer.is_writing() {
                if let Some(entries) = self.weapon_recoil_info.get(slot) {
                    for entry in entries.iter().take(recoil_info_count as usize) {
                        let mut state_value = recoil_state_to_i32(entry.state);
                        let mut shift = entry.shift;
                        let mut recoil_rate = entry.recoil_rate;
                        xfer.xfer_int(&mut state_value).map_err(|e| e.to_string())?;
                        xfer.xfer_real(&mut shift).map_err(|e| e.to_string())?;
                        xfer.xfer_real(&mut recoil_rate)
                            .map_err(|e| e.to_string())?;
                    }
                }
            } else {
                if let Some(entries) = self.weapon_recoil_info.get_mut(slot) {
                    entries.clear();
                    for _ in 0..recoil_info_count {
                        let mut state_value = 0i32;
                        let mut shift = 0.0f32;
                        let mut recoil_rate = 0.0f32;
                        xfer.xfer_int(&mut state_value).map_err(|e| e.to_string())?;
                        xfer.xfer_real(&mut shift).map_err(|e| e.to_string())?;
                        xfer.xfer_real(&mut recoil_rate)
                            .map_err(|e| e.to_string())?;
                        entries.push(WeaponRecoilInfo {
                            state: recoil_state_from_i32(state_value),
                            shift,
                            recoil_rate,
                        });
                    }
                }
            }
        }

        let mut sub_object_count = self.sub_object_vec.len().min(u8::MAX as usize) as u8;
        xfer.xfer_unsigned_byte(&mut sub_object_count)
            .map_err(|e| e.to_string())?;
        if xfer.is_writing() {
            for sub_obj in self.sub_object_vec.iter().take(sub_object_count as usize) {
                let mut sub_obj_name = sub_obj.sub_obj_name.as_str().to_string();
                let mut hide = sub_obj.hide;
                xfer.xfer_ascii_string(&mut sub_obj_name)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut hide).map_err(|e| e.to_string())?;
            }
        } else {
            self.sub_object_vec.clear();
            for _ in 0..sub_object_count {
                let mut sub_obj_name = String::new();
                let mut hide = false;
                xfer.xfer_ascii_string(&mut sub_obj_name)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut hide).map_err(|e| e.to_string())?;
                self.sub_object_vec.push(HideShowSubObjInfo {
                    sub_obj_name: AsciiString::from(sub_obj_name.as_str()),
                    hide,
                });
            }
        }

        if version >= 2 {
            if xfer.is_writing() {
                let mut animation_payload_present =
                    self.which_anim_in_cur_state >= 0 && !self.is_current_transition_state();
                xfer.xfer_bool(&mut animation_payload_present)
                    .map_err(|e| e.to_string())?;
                if animation_payload_present {
                    let mut mode = self
                        .current_state()
                        .map(|state| anim_mode_to_i32(state.anim_mode))
                        .unwrap_or(0);
                    xfer.xfer_int(&mut mode).map_err(|e| e.to_string())?;

                    let mut percent = if self.current_anim_num_frames > 1 {
                        self.current_anim_frame as Real / (self.current_anim_num_frames - 1) as Real
                    } else {
                        0.0
                    };
                    xfer.xfer_real(&mut percent).map_err(|e| e.to_string())?;
                }
            } else {
                let mut animation_payload_present = false;
                xfer.xfer_bool(&mut animation_payload_present)
                    .map_err(|e| e.to_string())?;
                if animation_payload_present {
                    let mut ignored_mode = 0i32;
                    xfer.xfer_int(&mut ignored_mode)
                        .map_err(|e| e.to_string())?;
                    let mut percent = 0.0f32;
                    xfer.xfer_real(&mut percent).map_err(|e| e.to_string())?;
                    if self.current_anim_num_frames > 1 {
                        let frame =
                            (percent * (self.current_anim_num_frames - 1) as Real).round() as i32;
                        self.current_anim_frame = frame.clamp(0, self.current_anim_num_frames - 1);
                    } else {
                        self.current_anim_frame = 0;
                    }
                    self.current_anim_complete = false;
                }
            }
        }

        if xfer.is_reading() && !self.sub_object_vec.is_empty() {
            self.update_sub_objects();
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if !self.sub_object_vec.is_empty() {
            self.update_sub_objects();
        }
        Ok(())
    }
}
