// ScriptEngine xfer helpers and snapshot leftover
//
// Split from `scripting/engine.rs` for module-size parity.
// Observable behavior is unchanged.

fn xfer_list_ascii_string(xfer: &mut dyn Xfer, list: &mut Vec<String>) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut count: u16 = if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
        list.len() as u16
    } else {
        0
    };
    xfer.xfer_unsigned_short(&mut count)?;

    match xfer.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            for entry in list.iter() {
                let mut value = entry.clone();
                xfer.xfer_ascii_string(&mut value)?;
            }
        }
        XferMode::Load => {
            if !list.is_empty() {
                return Err(XferStatus::ListNotEmpty);
            }
            list.clear();
            for _ in 0..count {
                let mut value = String::new();
                xfer.xfer_ascii_string(&mut value)?;
                list.push(value);
            }
        }
        XferMode::Invalid => return Err(XferStatus::ModeUnknown),
    }

    Ok(())
}

fn xfer_list_ascii_string_uint(
    xfer: &mut dyn Xfer,
    list: &mut Vec<(String, u32)>,
) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut count: u16 = if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
        list.len() as u16
    } else {
        0
    };
    xfer.xfer_unsigned_short(&mut count)?;

    match xfer.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            for (name, value) in list.iter() {
                let mut entry_name = name.clone();
                let mut entry_value = *value;
                xfer.xfer_ascii_string(&mut entry_name)?;
                xfer.xfer_unsigned_int(&mut entry_value)?;
            }
        }
        XferMode::Load => {
            if !list.is_empty() {
                return Err(XferStatus::ListNotEmpty);
            }
            list.clear();
            for _ in 0..count {
                let mut entry_name = String::new();
                let mut entry_value: u32 = 0;
                xfer.xfer_ascii_string(&mut entry_name)?;
                xfer.xfer_unsigned_int(&mut entry_value)?;
                list.push((entry_name, entry_value));
            }
        }
        XferMode::Invalid => return Err(XferStatus::ModeUnknown),
    }

    Ok(())
}

fn xfer_list_ascii_string_object_id(
    xfer: &mut dyn Xfer,
    list: &mut Vec<(String, ObjectID)>,
) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut count: u16 = if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
        list.len() as u16
    } else {
        0
    };
    xfer.xfer_unsigned_short(&mut count)?;

    match xfer.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            for (name, object_id) in list.iter() {
                let mut entry_name = name.clone();
                let mut entry_id = *object_id;
                xfer.xfer_ascii_string(&mut entry_name)?;
                xfer.xfer_object_id(&mut entry_id)?;
            }
        }
        XferMode::Load => {
            if !list.is_empty() {
                return Err(XferStatus::ListNotEmpty);
            }
            list.clear();
            for _ in 0..count {
                let mut entry_name = String::new();
                let mut entry_id: ObjectID = crate::common::INVALID_ID;
                xfer.xfer_ascii_string(&mut entry_name)?;
                xfer.xfer_object_id(&mut entry_id)?;
                list.push((entry_name, entry_id));
            }
        }
        XferMode::Invalid => return Err(XferStatus::ModeUnknown),
    }

    Ok(())
}

fn xfer_list_ascii_string_coord3d(
    xfer: &mut dyn Xfer,
    list: &mut Vec<(String, Coord3D)>,
) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut count: u16 = if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
        list.len() as u16
    } else {
        0
    };
    xfer.xfer_unsigned_short(&mut count)?;

    match xfer.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            for (name, coord) in list.iter() {
                let mut entry_name = name.clone();
                let mut entry_coord = *coord;
                xfer.xfer_ascii_string(&mut entry_name)?;
                xfer.xfer_real(&mut entry_coord.x)?;
                xfer.xfer_real(&mut entry_coord.y)?;
                xfer.xfer_real(&mut entry_coord.z)?;
            }
        }
        XferMode::Load => {
            if !list.is_empty() {
                return Err(XferStatus::ListNotEmpty);
            }
            list.clear();
            for _ in 0..count {
                let mut entry_name = String::new();
                let mut entry_coord = Coord3D::zero();
                xfer.xfer_ascii_string(&mut entry_name)?;
                xfer.xfer_real(&mut entry_coord.x)?;
                xfer.xfer_real(&mut entry_coord.y)?;
                xfer.xfer_real(&mut entry_coord.z)?;
                list.push((entry_name, entry_coord));
            }
        }
        XferMode::Invalid => return Err(XferStatus::ModeUnknown),
    }

    Ok(())
}

fn xfer_science_vec(xfer: &mut dyn Xfer, list: &mut Vec<ScienceType>) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut count: u16 = if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
        list.len() as u16
    } else {
        0
    };
    xfer.xfer_unsigned_short(&mut count)?;

    match xfer.get_xfer_mode() {
        XferMode::Save => {
            for science in list.iter() {
                let mut value = *science as i32;
                xfer.xfer_int(&mut value)?;
            }
        }
        XferMode::Load => {
            list.clear();
            for _ in 0..count {
                let mut value: i32 = 0;
                xfer.xfer_int(&mut value)?;
                list.push(value as ScienceType);
            }
        }
        XferMode::Crc => {
            for science in list.iter() {
                let mut value = *science as i32;
                xfer.xfer_int(&mut value)?;
            }
        }
        XferMode::Invalid => return Err(XferStatus::ModeUnknown),
    }

    Ok(())
}

impl XferSnapshot for ScriptEngine {
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let inner = self.inner.get_mut();
        let current_version: XferVersion = 6;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        let mut sequential_script_count: u16 =
            if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
                inner.sequential_scripts.len() as u16
            } else {
                0
            };
        xfer.xfer_unsigned_short(&mut sequential_script_count)?;

        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                for script in inner.sequential_scripts.iter_mut() {
                    script.xfer(xfer)?;
                }
            }
            XferMode::Load => {
                if !inner.sequential_scripts.is_empty() {
                    return Err(XferStatus::ListNotEmpty);
                }
                inner.sequential_scripts.clear();
                for _ in 0..sequential_script_count {
                    let mut script = SequentialScript::new();
                    script.xfer(xfer)?;
                    inner.sequential_scripts.push(script);
                }
            }
            XferMode::Invalid => return Err(XferStatus::ModeUnknown),
        }

        let mut counters_size: u16 =
            if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
                inner.num_counters as u16
            } else {
                0
            };
        xfer.xfer_unsigned_short(&mut counters_size)?;
        if counters_size as usize > MAX_COUNTERS {
            return Err(XferStatus::InvalidParameters);
        }
        for i in 0..counters_size as usize {
            if xfer.get_xfer_mode() == XferMode::Load && inner.counters[i].is_none() {
                inner.counters[i] = Some(TCounter::new(String::new()));
            }
            let Some(counter) = inner.counters[i].as_mut() else {
                return Err(XferStatus::InvalidParameters);
            };
            xfer.xfer_int(&mut counter.value)?;
            xfer.xfer_ascii_string(&mut counter.name)?;
            xfer.xfer_bool(&mut counter.is_countdown_timer)?;
        }

        let mut num_counters = inner.num_counters as i32;
        xfer.xfer_int(&mut num_counters)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            inner.num_counters = num_counters as usize;
        }

        let mut flags_size: u16 = if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc)
        {
            inner.num_flags as u16
        } else {
            0
        };
        xfer.xfer_unsigned_short(&mut flags_size)?;
        if flags_size as usize > MAX_FLAGS {
            return Err(XferStatus::InvalidParameters);
        }
        for i in 0..flags_size as usize {
            if xfer.get_xfer_mode() == XferMode::Load && inner.flags[i].is_none() {
                inner.flags[i] = Some(TFlag::new(String::new()));
            }
            let Some(flag) = inner.flags[i].as_mut() else {
                return Err(XferStatus::InvalidParameters);
            };
            xfer.xfer_bool(&mut flag.value)?;
            xfer.xfer_ascii_string(&mut flag.name)?;
        }

        let mut num_flags = inner.num_flags as i32;
        xfer.xfer_int(&mut num_flags)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            inner.num_flags = num_flags as usize;
        }

        let mut attack_priority_size: u16 =
            if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
                inner.num_attack_info as u16
            } else {
                0
            };
        xfer.xfer_unsigned_short(&mut attack_priority_size)?;
        if attack_priority_size as usize > MAX_ATTACK_PRIORITIES {
            return Err(XferStatus::InvalidParameters);
        }
        if xfer.get_xfer_mode() == XferMode::Load {
            inner.attack_priority_info.clear();
            inner.attack_priority_info
                .resize_with(attack_priority_size as usize, AttackPriorityInfo::new);
        }
        for i in 0..attack_priority_size as usize {
            inner.attack_priority_info[i].xfer(xfer)?;
        }

        let mut num_attack_info = inner.num_attack_info as i32;
        xfer.xfer_int(&mut num_attack_info)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            inner.num_attack_info = num_attack_info as usize;
        }

        if version >= 6 {
            let mut object_priority_count: u16 =
                if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
                    inner.object_attack_priority_sets
                        .len()
                        .min(u16::MAX as usize) as u16
                } else {
                    0
                };
            xfer.xfer_unsigned_short(&mut object_priority_count)?;

            match xfer.get_xfer_mode() {
                XferMode::Save | XferMode::Crc => {
                    let mut entries: Vec<(ObjectID, String)> = inner
                        .object_attack_priority_sets
                        .iter()
                        .map(|(object_id, set_name)| (*object_id, set_name.clone()))
                        .collect();
                    entries.sort_by_key(|(object_id, _)| *object_id);
                    for (object_id, mut set_name) in entries {
                        let mut object_id = object_id;
                        xfer.xfer_object_id(&mut object_id)?;
                        xfer.xfer_ascii_string(&mut set_name)?;
                    }
                }
                XferMode::Load => {
                    inner.object_attack_priority_sets.clear();
                    for _ in 0..object_priority_count {
                        let mut object_id: ObjectID = crate::common::INVALID_ID;
                        let mut set_name = String::new();
                        xfer.xfer_object_id(&mut object_id)?;
                        xfer.xfer_ascii_string(&mut set_name)?;
                        if object_id == crate::common::INVALID_ID || set_name.is_empty() {
                            continue;
                        }
                        inner.object_attack_priority_sets.insert(object_id, set_name);
                    }
                }
                XferMode::Invalid => return Err(XferStatus::ModeUnknown),
            }
        } else if xfer.get_xfer_mode() == XferMode::Load {
            inner.object_attack_priority_sets.clear();
        }

        xfer.xfer_int(&mut inner.end_game_timer)?;
        xfer.xfer_int(&mut inner.close_window_timer)?;

        let named_object_tracker = get_named_object_tracker();
        let named_objects: Vec<(String, ObjectID)> =
            if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
                let mut entries = named_object_tracker
                    .get_all_named_objects()
                    .unwrap_or_default();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                entries
            } else {
                Vec::new()
            };
        let mut named_objects_count: u16 =
            if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
                named_objects.len() as u16
            } else {
                0
            };
        xfer.xfer_unsigned_short(&mut named_objects_count)?;

        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                for (name, object_id) in named_objects.iter() {
                    let mut entry_name = name.clone();
                    let mut entry_id = *object_id;
                    xfer.xfer_ascii_string(&mut entry_name)?;
                    xfer.xfer_object_id(&mut entry_id)?;
                }
            }
            XferMode::Load => {
                named_object_tracker
                    .clear()
                    .map_err(|_| XferStatus::ErrorUnknown)?;
                for _ in 0..named_objects_count {
                    let mut entry_name = String::new();
                    let mut entry_id: ObjectID = crate::common::INVALID_ID;
                    xfer.xfer_ascii_string(&mut entry_name)?;
                    xfer.xfer_object_id(&mut entry_id)?;
                    if entry_id != crate::common::INVALID_ID
                        && OBJECT_REGISTRY.with_object(entry_id, |_| ()).is_none()
                    {
                        return Err(XferStatus::InvalidParameters);
                    }
                    named_object_tracker
                        .register_named_object(entry_name, entry_id)
                        .map_err(|_| XferStatus::ErrorUnknown)?;
                }
            }
            XferMode::Invalid => return Err(XferStatus::ModeUnknown),
        }

        xfer.xfer_bool(&mut inner.first_update)?;

        let mut fade_value: i32 = match inner.fade {
            TFade::None => 0,
            TFade::Subtract => 1,
            TFade::Add => 2,
            TFade::Saturate => 3,
            TFade::Multiply => 4,
        };
        xfer.xfer_int(&mut fade_value)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            inner.fade = match fade_value {
                0 => TFade::None,
                1 => TFade::Subtract,
                2 => TFade::Add,
                3 => TFade::Saturate,
                4 => TFade::Multiply,
                _ => return Err(XferStatus::InvalidParameters),
            };
        }

        xfer.xfer_real(&mut inner.min_fade)?;
        xfer.xfer_real(&mut inner.max_fade)?;
        xfer.xfer_real(&mut inner.cur_fade_value)?;
        xfer.xfer_int(&mut inner.cur_fade_frame)?;
        xfer.xfer_int(&mut inner.fade_frames_increase)?;
        xfer.xfer_int(&mut inner.fade_frames_hold)?;
        xfer.xfer_int(&mut inner.fade_frames_decrease)?;

        xfer_list_ascii_string(xfer, &mut inner.completed_video)?;
        xfer_list_ascii_string_uint(xfer, &mut inner.testing_speech)?;
        xfer_list_ascii_string_uint(xfer, &mut inner.testing_audio)?;
        xfer_list_ascii_string(xfer, &mut inner.ui_interactions)?;

        let mut triggered_special_powers_size: u16 = Self::MAX_PLAYER_COUNT as u16;
        xfer.xfer_unsigned_short(&mut triggered_special_powers_size)?;
        if triggered_special_powers_size != Self::MAX_PLAYER_COUNT as u16 {
            return Err(XferStatus::InvalidParameters);
        }
        for i in 0..triggered_special_powers_size as usize {
            xfer_list_ascii_string_object_id(xfer, &mut inner.triggered_special_powers[i])?;
        }

        let mut midway_special_powers_size: u16 = Self::MAX_PLAYER_COUNT as u16;
        xfer.xfer_unsigned_short(&mut midway_special_powers_size)?;
        if midway_special_powers_size != Self::MAX_PLAYER_COUNT as u16 {
            return Err(XferStatus::InvalidParameters);
        }
        for i in 0..midway_special_powers_size as usize {
            xfer_list_ascii_string_object_id(xfer, &mut inner.midway_special_powers[i])?;
        }

        let mut finished_special_powers_size: u16 = Self::MAX_PLAYER_COUNT as u16;
        xfer.xfer_unsigned_short(&mut finished_special_powers_size)?;
        if finished_special_powers_size != Self::MAX_PLAYER_COUNT as u16 {
            return Err(XferStatus::InvalidParameters);
        }
        for i in 0..finished_special_powers_size as usize {
            xfer_list_ascii_string_object_id(xfer, &mut inner.finished_special_powers[i])?;
        }

        let mut completed_upgrades_size: u16 = Self::MAX_PLAYER_COUNT as u16;
        xfer.xfer_unsigned_short(&mut completed_upgrades_size)?;
        if completed_upgrades_size != Self::MAX_PLAYER_COUNT as u16 {
            return Err(XferStatus::InvalidParameters);
        }
        for i in 0..completed_upgrades_size as usize {
            xfer_list_ascii_string_object_id(xfer, &mut inner.completed_upgrades[i])?;
        }

        let mut acquired_sciences_size: u16 = Self::MAX_PLAYER_COUNT as u16;
        xfer.xfer_unsigned_short(&mut acquired_sciences_size)?;
        if acquired_sciences_size != Self::MAX_PLAYER_COUNT as u16 {
            return Err(XferStatus::InvalidParameters);
        }
        for i in 0..acquired_sciences_size as usize {
            xfer_science_vec(xfer, &mut inner.acquired_sciences[i])?;
        }

        xfer_list_ascii_string_coord3d(xfer, &mut inner.topple_directions)?;

        xfer.xfer_real(&mut inner.breeze_info.direction)?;
        xfer.xfer_real(&mut inner.breeze_info.direction_vec[0])?;
        xfer.xfer_real(&mut inner.breeze_info.direction_vec[1])?;
        xfer.xfer_real(&mut inner.breeze_info.intensity)?;
        xfer.xfer_real(&mut inner.breeze_info.lean)?;
        xfer.xfer_real(&mut inner.breeze_info.randomness)?;
        xfer.xfer_short(&mut inner.breeze_info.breeze_period)?;
        xfer.xfer_short(&mut inner.breeze_info.breeze_version)?;

        let mut difficulty_value: i32 = match inner.game_difficulty {
            crate::player::GameDifficulty::Easy => 0,
            crate::player::GameDifficulty::Normal => 1,
            crate::player::GameDifficulty::Hard => 2,
            crate::player::GameDifficulty::Brutal => 3,
        };
        xfer.xfer_int(&mut difficulty_value)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            inner.game_difficulty = match difficulty_value {
                0 => crate::player::GameDifficulty::Easy,
                1 => crate::player::GameDifficulty::Normal,
                2 => crate::player::GameDifficulty::Hard,
                3 => crate::player::GameDifficulty::Brutal,
                _ => return Err(XferStatus::InvalidParameters),
            };
        }

        xfer.xfer_bool(&mut inner.freeze_by_script)?;

        if version >= 2 {
            let mut named_reveal_count: u16 =
                if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
                    inner.named_reveals.len() as u16
                } else {
                    0
                };
            xfer.xfer_unsigned_short(&mut named_reveal_count)?;
            match xfer.get_xfer_mode() {
                XferMode::Save | XferMode::Crc => {
                    for reveal in inner.named_reveals.iter_mut() {
                        xfer.xfer_ascii_string(&mut reveal.reveal_name)?;
                        xfer.xfer_ascii_string(&mut reveal.waypoint_name)?;
                        xfer.xfer_real(&mut reveal.radius_to_reveal)?;
                        xfer.xfer_ascii_string(&mut reveal.player_name)?;
                    }
                }
                XferMode::Load => {
                    if !inner.named_reveals.is_empty() {
                        return Err(XferStatus::ListNotEmpty);
                    }
                    inner.named_reveals.clear();
                    for _ in 0..named_reveal_count {
                        let mut reveal = NamedReveal {
                            reveal_name: String::new(),
                            waypoint_name: String::new(),
                            radius_to_reveal: 0.0,
                            player_name: String::new(),
                        };
                        xfer.xfer_ascii_string(&mut reveal.reveal_name)?;
                        xfer.xfer_ascii_string(&mut reveal.waypoint_name)?;
                        xfer.xfer_real(&mut reveal.radius_to_reveal)?;
                        xfer.xfer_ascii_string(&mut reveal.player_name)?;
                        inner.named_reveals.push(reveal);
                    }
                }
                XferMode::Invalid => return Err(XferStatus::ModeUnknown),
            }

            let mut all_object_types_count: u16 =
                if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
                    inner.object_types.len() as u16
                } else {
                    0
                };
            xfer.xfer_unsigned_short(&mut all_object_types_count)?;

            match xfer.get_xfer_mode() {
                XferMode::Save | XferMode::Crc => {
                    let mut ordered_lists: Vec<&ObjectTypes> = inner.object_types.values().collect();
                    ordered_lists
                        .sort_by(|a, b| a.list_name().as_str().cmp(b.list_name().as_str()));
                    for entry in ordered_lists.iter() {
                        let current_version: XferVersion = 1;
                        let mut obj_version = current_version;
                        xfer.xfer_version(&mut obj_version, current_version)?;

                        let mut list_name = entry.list_name().as_str().to_string();
                        xfer.xfer_ascii_string(&mut list_name)?;

                        let mut object_type_count: u16 = entry.list_size() as u16;
                        xfer.xfer_unsigned_short(&mut object_type_count)?;
                        for object_type in entry.iter() {
                            let mut object_type_name = object_type.as_str().to_string();
                            xfer.xfer_ascii_string(&mut object_type_name)?;
                        }
                    }
                }
                XferMode::Load => {
                    if !inner.object_types.is_empty() {
                        return Err(XferStatus::ListNotEmpty);
                    }
                    inner.object_types.clear();
                    for _ in 0..all_object_types_count {
                        let current_version: XferVersion = 1;
                        let mut obj_version = current_version;
                        xfer.xfer_version(&mut obj_version, current_version)?;

                        let mut list_name = String::new();
                        xfer.xfer_ascii_string(&mut list_name)?;

                        let mut object_type_count: u16 = 0;
                        xfer.xfer_unsigned_short(&mut object_type_count)?;

                        let mut list =
                            ObjectTypes::with_list_name(AsciiString::from(list_name.as_str()));
                        for _ in 0..object_type_count {
                            let mut object_type_name = String::new();
                            xfer.xfer_ascii_string(&mut object_type_name)?;
                            list.add_object_type(AsciiString::from(object_type_name.as_str()));
                        }

                        let key = list.list_name().as_str().to_string();
                        inner.object_types.insert(key, list);
                    }
                }
                XferMode::Invalid => return Err(XferStatus::ModeUnknown),
            }
        }

        if version >= 3 {
            xfer.xfer_bool(&mut inner.objects_should_receive_difficulty_bonus)?;
        } else {
            inner.objects_should_receive_difficulty_bonus = true;
        }

        if version >= 4 {
            xfer.xfer_ascii_string(&mut inner.current_track_name)?;
        }

        if version >= 5 {
            xfer.xfer_bool(&mut inner.choose_victim_always_uses_normal)?;
        } else {
            inner.choose_victim_always_uses_normal = false;
        }

        if xfer.get_xfer_mode() == XferMode::Load && inner.fade == TFade::None {
            inner.fade = TFade::Multiply;
            inner.cur_fade_frame = 0;
            inner.min_fade = 1.0;
            inner.max_fade = 0.0;
            inner.fade_frames_increase = 0;
            inner.fade_frames_hold = 0;
            inner.fade_frames_decrease = FRAMES_TO_FADE_IN_AT_START;
            inner.cur_fade_value = 0.0;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}
