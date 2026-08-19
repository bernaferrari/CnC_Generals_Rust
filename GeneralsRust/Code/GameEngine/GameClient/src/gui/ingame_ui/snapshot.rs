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
        // C++ InGameUI::xfer (InGameUI.cpp:340-503): version 3.
        // named timers (v2+), m_superweaponHiddenByScript, then SW entries
        // {playerIndex, templateName, powerName, ObjectID, timestamp,
        //  hiddenByScript, hiddenByScience, ready, evaReadyPlayed} ended by -1.
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

        // C++: xfer->xferBool(&m_superweaponHiddenByScript) (InGameUI.cpp:387)
        xfer.xfer_bool(&mut self.superweapon_hidden_by_script)
            .map_err(|e| e.to_string())?;

        if xfer.is_writing() {
            for timer in &self.superweapon_timers {
                let mut player_index = timer.player_index as i32;
                let mut template_name = if timer.template_name.is_empty() {
                    timer.power_name.clone()
                } else {
                    timer.template_name.clone()
                };
                let mut power_name = timer.power_name.clone();
                let mut object_id = timer.object_id;
                let mut timestamp = timer.timestamp.max(0) as u32;
                let mut hidden_by_script = timer.hidden_by_script;
                let mut hidden_by_science = timer.hidden_by_science;
                let mut ready = timer.ready;
                let mut eva_ready_played = timer.eva_ready_played;

                xfer.xfer_int(&mut player_index)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_ascii_string(&mut template_name)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_ascii_string(&mut power_name)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_u32(&mut object_id).map_err(|e| e.to_string())?;
                xfer.xfer_u32(&mut timestamp).map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut hidden_by_script)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut hidden_by_science)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut ready).map_err(|e| e.to_string())?;
                if version >= 3 {
                    xfer.xfer_bool(&mut eva_ready_played)
                        .map_err(|e| e.to_string())?;
                }
            }
            let mut sentinel: i32 = -1;
            xfer.xfer_int(&mut sentinel).map_err(|e| e.to_string())?;
        } else if xfer.is_reading() {
            loop {
                let mut player_index: i32 = 0;
                xfer.xfer_int(&mut player_index)
                    .map_err(|e| e.to_string())?;
                if player_index == -1 {
                    break;
                }
                if player_index < 0 || player_index >= MAX_PLAYER_COUNT as i32 {
                    return Err("SWInfo bad plyrindex".to_string());
                }

                let mut template_name = String::new();
                let mut power_name = String::new();
                let mut object_id: u32 = 0;
                let mut timestamp: u32 = 0;
                let mut hidden_by_script = false;
                let mut hidden_by_science = false;
                let mut ready = false;
                let mut eva_ready_played = false;

                xfer.xfer_ascii_string(&mut template_name)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_ascii_string(&mut power_name)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_u32(&mut object_id).map_err(|e| e.to_string())?;
                xfer.xfer_u32(&mut timestamp).map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut hidden_by_script)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut hidden_by_science)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut ready).map_err(|e| e.to_string())?;
                if version >= 3 {
                    xfer.xfer_bool(&mut eva_ready_played)
                        .map_err(|e| e.to_string())?;
                } else {
                    eva_ready_played = ready;
                }

                if let Some(existing) = self.superweapon_timers.iter_mut().find(|timer| {
                    timer.player_index == player_index as u8
                        && timer.power_name == power_name
                        && timer.object_id == object_id
                }) {
                    existing.template_name = template_name;
                    existing.timestamp = timestamp as i32;
                    existing.hidden_by_script = hidden_by_script;
                    existing.hidden_by_science = hidden_by_science;
                    existing.ready = ready;
                    existing.eva_ready_played = eva_ready_played;
                    existing.force_update_text = true;
                } else {
                    self.superweapon_timers.push(SuperweaponTimerData {
                        player_index: player_index as u8,
                        object_id,
                        power_name,
                        template_name,
                        ready_frame: 0,
                        countdown_text: String::new(),
                        ready,
                        hidden_by_script,
                        hidden_by_science,
                        timestamp: timestamp as i32,
                        eva_ready_played,
                        color: 0xFFFFFFFF,
                        force_update_text: true,
                        name_text: String::new(),
                        time_text: String::new(),
                        use_ready_font: ready,
                    });
                }
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> std::result::Result<(), String> {
        Ok(())
    }
}

