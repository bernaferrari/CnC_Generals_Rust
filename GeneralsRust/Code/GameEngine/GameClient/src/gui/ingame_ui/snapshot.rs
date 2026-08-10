// Snapshotable save/load for InGameUI.
// Split from `gui/ingame_ui.rs` dump. Included by `ingame_ui/mod.rs`.

impl Snapshotable for InGameUI {
    fn crc(&self, xfer: &mut dyn Xfer) -> std::result::Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> std::result::Result<(), String> {
        let current_version: XferVersion = 3;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        if version >= 2 {
            xfer.xfer_int(&mut self.named_timer_last_flash_frame)
                .map_err(|e| e.to_string())?;
            xfer.xfer_bool(&mut self.named_timer_used_flash_color)
                .map_err(|e| e.to_string())?;
            xfer.xfer_bool(&mut self.show_named_timers)
                .map_err(|e| e.to_string())?;

            let mut timer_count = self.named_timers.len() as i32;
            xfer.xfer_int(&mut timer_count).map_err(|e| e.to_string())?;

            if xfer.is_writing() {
                for timer in self.named_timers.iter() {
                    let mut name = timer.name.clone();
                    let mut text = timer.text.clone();
                    let mut is_countdown = timer.is_countdown;
                    xfer.xfer_ascii_string(&mut name)
                        .map_err(|e| e.to_string())?;
                    xfer.xfer_unicode_string(&mut text)
                        .map_err(|e| e.to_string())?;
                    xfer.xfer_bool(&mut is_countdown)
                        .map_err(|e| e.to_string())?;
                }
            } else if xfer.is_reading() {
                self.named_timers.clear();
                for _ in 0..timer_count {
                    let mut name = String::new();
                    let mut text = String::new();
                    let mut is_countdown = false;
                    xfer.xfer_ascii_string(&mut name)
                        .map_err(|e| e.to_string())?;
                    xfer.xfer_unicode_string(&mut text)
                        .map_err(|e| e.to_string())?;
                    xfer.xfer_bool(&mut is_countdown)
                        .map_err(|e| e.to_string())?;
                    self.add_named_timer(&name, text, is_countdown);
                }
            }
        }

        xfer.xfer_bool(&mut self.quit_menu_visible)
            .map_err(|e| e.to_string())?;

        // C++: xfer->xferBool(&m_superweaponHiddenByScript) (InGameUI.cpp:387)
        xfer.xfer_bool(&mut self.superweapon_hidden_by_script)
            .map_err(|e| e.to_string())?;

        // Save/restore selection list (object IDs)
        let mut selection_count = self.get_selection().len() as i32;
        xfer.xfer_int(&mut selection_count)
            .map_err(|e| e.to_string())?;

        if xfer.is_writing() {
            for obj_id in self.get_selection() {
                let mut id = obj_id;
                xfer.xfer_u32(&mut id).map_err(|e| e.to_string())?;
            }
        } else if xfer.is_reading() {
            let mut ids: Vec<ObjectID> = Vec::with_capacity(selection_count.max(0) as usize);
            for _ in 0..selection_count.max(0) {
                let mut id: u32 = 0;
                xfer.xfer_u32(&mut id).map_err(|e| e.to_string())?;
                ids.push(id);
            }
            let selection_manager = get_selection_manager();
            if let Ok(mut manager) = selection_manager.write() {
                if let Some(state) = manager.get_player_selection(self.player_id as i32) {
                    state.select_objects(ids, SelectionType::Replace);
                }
            }
            self.sync_selection_state();
        }

        // Save/restore control groups (10 groups, each a list of object IDs)
        for group_idx in 0..10usize {
            let group = self.get_control_group(group_idx as i32);
            let mut count = group.len() as i32;
            xfer.xfer_int(&mut count).map_err(|e| e.to_string())?;

            if xfer.is_writing() {
                for obj_id in group {
                    let mut id = obj_id;
                    xfer.xfer_u32(&mut id).map_err(|e| e.to_string())?;
                }
            } else if xfer.is_reading() {
                let mut group_ids: Vec<ObjectID> = Vec::with_capacity(count.max(0) as usize);
                for _ in 0..count.max(0) {
                    let mut id: u32 = 0;
                    xfer.xfer_u32(&mut id).map_err(|e| e.to_string())?;
                    group_ids.push(id);
                }
                let sm = get_selection_manager();
                let Ok(mut manager) = sm.write() else {
                    continue;
                };
                if let Some(state) = manager.get_player_selection(self.player_id as i32) {
                    state.set_control_group_objects(group_idx, group_ids);
                }
            }
        }

        // Save/restore superweapon timer data
        // C++: iterates m_superweapons[playerIndex][powerName] list, saves per-entry
        if xfer.is_writing() {
            let mut sw_count = self.superweapon_timers.len() as i32;
            xfer.xfer_int(&mut sw_count).map_err(|e| e.to_string())?;
            for timer in &self.superweapon_timers {
                let mut player_index = timer.player_index as i32;
                let mut object_id = timer.object_id;
                let mut ready_frame = timer.ready_frame;
                let mut hidden_by_script = timer.hidden_by_script;
                let mut hidden_by_science = timer.hidden_by_science;
                let mut ready = timer.ready;
                let mut power_name = timer.power_name.clone();

                xfer.xfer_int(&mut player_index)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_u32(&mut object_id).map_err(|e| e.to_string())?;
                xfer.xfer_ascii_string(&mut power_name)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_u32(&mut ready_frame).map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut hidden_by_script)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut hidden_by_science)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut ready).map_err(|e| e.to_string())?;
            }
        } else if xfer.is_reading() {
            let mut sw_count: i32 = 0;
            xfer.xfer_int(&mut sw_count).map_err(|e| e.to_string())?;
            self.superweapon_timers.clear();
            for _ in 0..sw_count.max(0) {
                let mut player_index: i32 = 0;
                let mut object_id: u32 = 0;
                let mut power_name = String::new();
                let mut ready_frame: u32 = 0;
                let mut hidden_by_script = false;
                let mut hidden_by_science = false;
                let mut ready = false;

                xfer.xfer_int(&mut player_index)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_u32(&mut object_id).map_err(|e| e.to_string())?;
                xfer.xfer_ascii_string(&mut power_name)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_u32(&mut ready_frame).map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut hidden_by_script)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut hidden_by_science)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut ready).map_err(|e| e.to_string())?;

                self.superweapon_timers.push(SuperweaponTimerData {
                    player_index: player_index as u8,
                    object_id,
                    power_name,
                    ready_frame,
                    countdown_text: String::new(),
                    ready,
                    hidden_by_script,
                    hidden_by_science,
                });
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> std::result::Result<(), String> {
        Ok(())
    }
}

