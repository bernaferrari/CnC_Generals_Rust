fn xfer_sorted_string_int_map(
    xfer: &mut dyn Xfer,
    map: &mut HashMap<String, Int>,
) -> Result<(), XferStatus> {
    let mut count = if xfer.get_xfer_mode() == XferMode::Load {
        0u32
    } else {
        map.len() as u32
    };
    xfer.xfer_unsigned_int(&mut count)?;

    match xfer.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            let mut entries: Vec<_> = map
                .iter()
                .map(|(key, value)| (key.clone(), *value))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (mut key, mut value) in entries {
                xfer.xfer_string(&mut key)?;
                xfer.xfer_int(&mut value)?;
            }
        }
        XferMode::Load => {
            map.clear();
            for _ in 0..count {
                let mut key = String::new();
                let mut value: Int = 0;
                xfer.xfer_string(&mut key)?;
                xfer.xfer_int(&mut value)?;
                map.insert(key, value);
            }
        }
        XferMode::Invalid => return Err(XferStatus::ModeUnknown),
    }

    Ok(())
}

fn xfer_control_bar_overrides(
    xfer: &mut dyn Xfer,
    map: &mut HashMap<String, Option<String>>,
) -> Result<(), XferStatus> {
    match xfer.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            let mut entries: Vec<_> = map
                .iter()
                .map(|(key, value)| (key.clone(), value.clone().unwrap_or_default()))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));

            for (mut key, mut value) in entries {
                xfer.xfer_string(&mut key)?;
                xfer.xfer_string(&mut value)?;
            }

            let mut empty = String::new();
            xfer.xfer_string(&mut empty)?;
        }
        XferMode::Load => {
            map.clear();
            loop {
                let mut key = String::new();
                xfer.xfer_string(&mut key)?;
                if key.is_empty() {
                    break;
                }

                let mut value = String::new();
                xfer.xfer_string(&mut value)?;
                let value = if value.is_empty() { None } else { Some(value) };
                map.insert(key, value);
            }
        }
        XferMode::Invalid => return Err(XferStatus::ModeUnknown),
    }

    Ok(())
}

fn control_bar_override_key(command_set_name: &str, slot: i32) -> Option<String> {
    if !(0..crate::command_button::MAX_COMMANDS_PER_SET as i32).contains(&slot) {
        return None;
    }

    let slot_prefix = char::from_u32('0' as u32 + slot as u32)?;
    Some(format!("{}{}", slot_prefix, command_set_name))
}

/// Bridges GameLogic's `game_engine::Xfer` (System::Xfer) onto Common's
/// `game_engine::system::Xfer` so Object/PolygonTrigger Snapshot impls can run.
struct CommonXferBridge<'a> {
    inner: &'a mut dyn Xfer,
}

fn map_common_xfer_mode(mode: XferMode) -> game_engine::common::system::xfer::XferMode {
    use game_engine::common::system::xfer::XferMode as CommonMode;
    match mode {
        XferMode::Invalid => CommonMode::Invalid,
        XferMode::Save => CommonMode::Save,
        XferMode::Load => CommonMode::Load,
        XferMode::Crc => CommonMode::Crc,
    }
}

fn common_xfer_status(err: XferStatus) -> game_engine::common::system::xfer::XferStatus {
    use game_engine::common::system::xfer::XferStatus as C;
    match err {
        XferStatus::Invalid => C::Invalid,
        XferStatus::Ok => C::Ok,
        XferStatus::Eof => C::Eof,
        XferStatus::FileNotFound => C::FileNotFound,
        XferStatus::FileNotOpen => C::FileNotOpen,
        XferStatus::FileAlreadyOpen => C::FileAlreadyOpen,
        XferStatus::ReadError => C::ReadError,
        XferStatus::WriteError => C::WriteError,
        XferStatus::ModeUnknown => C::ModeUnknown,
        XferStatus::SkipError => C::SkipError,
        XferStatus::BeginEndMismatch => C::BeginEndMismatch,
        XferStatus::OutOfMemory => C::OutOfMemory,
        XferStatus::StringError => C::StringError,
        XferStatus::InvalidVersion => C::InvalidVersion,
        XferStatus::InvalidParameters => C::InvalidParameters,
        XferStatus::ListNotEmpty => C::ListNotEmpty,
        XferStatus::UnknownString => C::UnknownString,
        XferStatus::InvalidData => C::InvalidData,
        _ => C::ErrorUnknown,
    }
}

impl game_engine::common::system::xfer::Xfer for CommonXferBridge<'_> {
    fn get_xfer_mode(&self) -> game_engine::common::system::xfer::XferMode {
        map_common_xfer_mode(self.inner.get_xfer_mode())
    }

    fn get_identifier(&self) -> &str {
        self.inner.get_identifier()
    }

    fn set_options(&mut self, options: u32) {
        self.inner.set_options(options);
    }

    fn clear_options(&mut self, options: u32) {
        self.inner.clear_options(options);
    }

    fn get_options(&self) -> u32 {
        self.inner.get_options()
    }

    fn open(
        &mut self,
        identifier: &str,
    ) -> Result<(), game_engine::common::system::xfer::XferStatus> {
        self.inner
            .open(identifier.to_string())
            .map_err(common_xfer_status)
    }

    fn close(&mut self) -> Result<(), game_engine::common::system::xfer::XferStatus> {
        self.inner.close().map_err(common_xfer_status)
    }

    fn begin_block(
        &mut self,
    ) -> Result<
        game_engine::common::system::xfer::XferBlockSize,
        game_engine::common::system::xfer::XferStatus,
    > {
        self.inner.begin_block().map_err(common_xfer_status)
    }

    fn end_block(&mut self) -> Result<(), game_engine::common::system::xfer::XferStatus> {
        self.inner.end_block().map_err(common_xfer_status)
    }

    fn skip(
        &mut self,
        data_size: i32,
    ) -> Result<(), game_engine::common::system::xfer::XferStatus> {
        self.inner.skip(data_size).map_err(common_xfer_status)
    }

    fn xfer_snapshot(
        &mut self,
        _snapshot: &mut dyn game_engine::common::system::snapshot::Snapshotable,
    ) -> Result<(), game_engine::common::system::xfer::XferStatus> {
        Ok(())
    }

    fn xfer_ascii_string(&mut self, ascii_string_data: &mut String) -> std::io::Result<()> {
        self.inner
            .xfer_ascii_string(ascii_string_data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e:?}")))
    }

    fn xfer_unicode_string(&mut self, unicode_string_data: &mut String) -> std::io::Result<()> {
        self.inner
            .xfer_unicode_string(unicode_string_data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e:?}")))
    }

    // SAFETY: pure forwarding: the raw pointer is passed straight to the
    // wrapped `Xfer`, which upholds the same validity contract for
    // `data_size` bytes that this method received from its own caller.
    unsafe fn xfer_implementation(
        &mut self,
        data: *mut u8,
        data_size: usize,
    ) -> std::io::Result<()> {
        self.inner
            .xfer_implementation(data, data_size)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e:?}")))
    }
}

fn xfer_object_snapshot(obj: &mut Object, xfer: &mut dyn Xfer) {
    let mut bridge = CommonXferBridge { inner: xfer };
    crate::common::types::Snapshot::xfer(obj, &mut bridge);
}

fn xfer_polygon_snapshot(poly: &mut crate::polygon_trigger::PolygonTrigger, xfer: &mut dyn Xfer) {
    let mut bridge = CommonXferBridge { inner: xfer };
    crate::common::types::Snapshot::xfer(poly, &mut bridge);
}


fn xfer_game_logic_state(logic: &mut GameLogic, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    // C++ GameLogic::xfer currentVersion = 10 (GameLogic.cpp).
    let current_version: XferVersion = 10;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    xfer.xfer_unsigned_int(&mut logic.frame)?;

    // C++ explicitly does NOT xfer m_nextObjectID here (game-state block owns it).
    logic.xfer_object_toc(xfer)?;

    if xfer.get_xfer_mode() == XferMode::Load {
        logic.prepare_logic_for_object_load();
    }

    let mut object_count = logic.all_objects.len() as UnsignedInt;
    xfer.xfer_unsigned_int(&mut object_count)?;
    if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
        let object_ids: Vec<ObjectID> = logic.all_objects.clone();
        for obj_id in object_ids {
            let Some(arc) = logic.objects.get(&obj_id).cloned() else {
                continue;
            };
            let tname = {
                let Ok(obj) = arc.read() else {
                    continue;
                };
                obj.get_template().get_name().to_string()
            };
            let Some(toc) = logic.find_toc_entry_by_name(&tname) else {
                return Err(XferStatus::InvalidData);
            };
            let mut toc_id = toc.id;
            xfer.xfer_unsigned_short(&mut toc_id)?;
            let _ = xfer.begin_block();
            if let Ok(mut obj) = arc.write() {
                xfer_object_snapshot(&mut obj, xfer);
            }
            xfer.end_block()?;
        }
    } else {
        xfer_game_logic_objects_load(logic, xfer, object_count)?;
    }


    xfer_campaign_manager_snapshot(xfer)?;
    xfer_cave_system_snapshot(xfer)?;

    if version >= 2 {
        xfer.xfer_bool(&mut logic.is_scoring_enabled)?;
    }

    if version >= 3 {
        xfer_polygon_triggers(xfer)?;
    }

    if version >= 5 {
        xfer.xfer_int(&mut logic.rank_level_limit)?;
    }

    if version >= 6 {
        xfer_the_sell_list(xfer)?;
    }

    if version >= 7 {
        xfer_buildable_overrides_sentinel(xfer, &mut logic.buildable_status_overrides)?;
    }

    if version >= 8 {
        xfer.xfer_bool(&mut logic.show_behind_building_markers)?;
        xfer.xfer_bool(&mut logic.draw_icon_ui)?;
        xfer.xfer_bool(&mut logic.show_dynamic_lod)?;
        let mut hulk_max_lifetime = crate::helpers::TheGameLogic::get_hulk_max_lifetime_override();
        xfer.xfer_int(&mut hulk_max_lifetime)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            crate::helpers::TheGameLogic::set_hulk_max_lifetime_override(hulk_max_lifetime);
        }
        xfer_control_bar_overrides(xfer, &mut logic.control_bar_overrides)?;
    }

    if version >= 9 {
        let mut rank_points = crate::helpers::TheGameLogic::get_rank_points_to_add_at_game_start();
        xfer.xfer_int(&mut rank_points)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            crate::helpers::TheGameLogic::set_rank_points_to_add_at_game_start(rank_points);
        }
    }

    if version >= 10 {
        xfer.xfer_unsigned_short(&mut logic.superweapon_restriction)?;
    } else if xfer.get_xfer_mode() == XferMode::Load {
        logic.superweapon_restriction = 0;
    }

    Ok(())
}


fn xfer_cave_system_snapshot(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    // C++ xferSnapshot(TheCaveSystem) — CaveSystem::xfer v1.
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    let cave = crate::system::cave_system::TheCaveSystem();
    let mut guard = cave.lock().map_err(|_| XferStatus::InvalidData)?;
    guard.xfer_game_logic(xfer)
}

fn xfer_polygon_triggers(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    let terrain = get_terrain_logic();
    let mut terrain_guard = terrain.write().map_err(|_| XferStatus::InvalidData)?;
    let list = terrain_guard.get_trigger_areas_mut();
    let sanity = list.len() as UnsignedInt;
    let mut trigger_count = sanity;
    xfer.xfer_unsigned_int(&mut trigger_count)?;
    if xfer.get_xfer_mode() == XferMode::Load && sanity != trigger_count {
        return Err(XferStatus::InvalidData);
    }
    let ids: Vec<i32> = if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
        list.get_triggers().iter().map(|t| t.get_id()).collect()
    } else {
        Vec::new()
    };
    if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
        for id in ids {
            let mut trigger_id = id;
            xfer.xfer_int(&mut trigger_id)?;
            if let Some(poly) = list.get_by_id_mut(id) {
                xfer_polygon_snapshot(poly, xfer);
            }
        }
    } else {
        for _ in 0..trigger_count {
            let mut trigger_id = 0i32;
            xfer.xfer_int(&mut trigger_id)?;
            if let Some(poly) = list.get_by_id_mut(trigger_id) {
                xfer_polygon_snapshot(poly, xfer);
            } else {
                return Err(XferStatus::InvalidData);
            }
        }
        pathfinder_new_map_after_polygon_load();
    }
    Ok(())
}

fn xfer_the_sell_list(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    // C++ TheBuildAssistant->xferTheSellList
    if game_engine::common::system::build_assistant::get_build_assistant().is_none() {
        game_engine::common::system::build_assistant::init_build_assistant();
    }
    let mut assistant = game_engine::common::system::build_assistant::get_build_assistant()
        .ok_or(XferStatus::InvalidData)?;
    let mut count = assistant.get_sell_list().len() as i32;
    xfer.xfer_int(&mut count)?;
    if xfer.get_xfer_mode() == XferMode::Load {
        assistant.reset();
        for _ in 0..count {
            let mut id: u32 = 0;
            let mut sell_frame: u32 = 0;
            xfer.xfer_object_id(&mut id)?;
            xfer.xfer_unsigned_int(&mut sell_frame)?;
            assistant.sell_object(
                &game_engine::common::system::build_assistant::Object {
                    id,
                    position: Default::default(),
                    orientation: 0.0,
                    command_set: None,
                },
                sell_frame,
            );
        }
    } else {
        let items: Vec<_> = assistant.get_sell_list().iter().cloned().collect();
        for mut info in items {
            xfer.xfer_object_id(&mut info.id)?;
            xfer.xfer_unsigned_int(&mut info.sell_frame)?;
        }
    }
    Ok(())
}

fn xfer_buildable_overrides_sentinel(
    xfer: &mut dyn Xfer,
    map: &mut HashMap<String, Int>,
) -> Result<(), XferStatus> {
    // C++ v>=7: name + BuildableStatus until empty name (no count prefix).
    match xfer.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            let mut entries: Vec<_> = map.iter().map(|(k, v)| (k.clone(), *v)).collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (mut name, mut status) in entries {
                xfer.xfer_ascii_string(&mut name)?;
                xfer.xfer_int(&mut status)?;
            }
            let mut empty = String::new();
            xfer.xfer_ascii_string(&mut empty)?;
        }
        XferMode::Load => {
            map.clear();
            loop {
                let mut name = String::new();
                xfer.xfer_ascii_string(&mut name)?;
                if name.is_empty() {
                    break;
                }
                let mut status: Int = 0;
                xfer.xfer_int(&mut status)?;
                map.insert(name, status);
            }
        }
        XferMode::Invalid => return Err(XferStatus::ModeUnknown),
    }
    Ok(())
}

fn xfer_partition_cell_shroud(
    xfer: &mut dyn Xfer,
    cell: &mut crate::system::shroud_manager::ShroudCellSnapshot,
) -> Result<(), XferStatus> {
    // C++ PartitionCell::xfer (PartitionManager.cpp:1488-1494):
    // u8 v1 + raw ShroudLevel[MAX_PLAYER_COUNT] (two Shorts per player).
    let mut cell_version: XferVersion = 1;
    xfer.xfer_version(&mut cell_version, 1)?;
    for player in 0..crate::common::MAX_PLAYER_COUNT {
        let mut current = cell.current_shroud[player] as i16;
        let mut active = cell.active_shroud_level[player] as i16;
        xfer.xfer_short(&mut current)?;
        xfer.xfer_short(&mut active)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            cell.current_shroud[player] = current as i32;
            cell.active_shroud_level[player] = active as i32;
        }
    }
    Ok(())
}

fn xfer_sighting_info(
    xfer: &mut dyn Xfer,
    info: &mut crate::system::shroud_manager::ShroudPendingUndoRevealSnapshot,
) -> Result<(), XferStatus> {
    // C++ SightingInfo::xfer (PartitionManager.cpp:5786-5805).
    let mut version: XferVersion = 1;
    xfer.xfer_version(&mut version, 1)?;
    xfer.xfer_real(&mut info.where_pos[0])?;
    xfer.xfer_real(&mut info.where_pos[1])?;
    xfer.xfer_real(&mut info.where_pos[2])?;
    xfer.xfer_real(&mut info.how_far)?;
    let mut for_whom = info.for_whom as u16;
    // SAFETY: `for_whom` is an initialized stack `u16`; `xfer_user` moves
    // exactly `size_of::<u16>()` bytes within this call (C++ SightingInfo
    // xferUser parity).
    unsafe {
        xfer.xfer_user((&mut for_whom as *mut u16).cast::<u8>(), std::mem::size_of::<u16>())?;
    }
    if xfer.get_xfer_mode() == XferMode::Load {
        info.for_whom = for_whom as u32;
    }
    xfer.xfer_unsigned_int(&mut info.expiration_frame)?;
    Ok(())
}

fn xfer_partition_state(logic: &mut GameLogic, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    // C++ PartitionManager::xfer (PartitionManager.cpp:4558-4657) currentVersion = 2.
    let current_version: XferVersion = 2;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let shroud_manager = get_shroud_manager();
    let mut shroud = shroud_manager.lock().map_err(|_| XferStatus::InvalidData)?;
    // C++ xfers into the already-allocated live cell array.
    let mut snapshot = shroud.snapshot_state();

    let mut cell_size = snapshot
        .grid
        .as_ref()
        .map(|grid| grid.cell_size)
        .unwrap_or(logic.partition_manager.cell_size);
    xfer.xfer_real(&mut cell_size)?;

    if let Some(grid) = snapshot.grid.as_ref() {
        if (cell_size - grid.cell_size).abs() > f32::EPSILON {
            return Err(XferStatus::InvalidData);
        }
    }

    let mut total_cell_count = snapshot
        .grid
        .as_ref()
        .map(|grid| grid.cells.len() as i32)
        .unwrap_or(0);
    xfer.xfer_int(&mut total_cell_count)?;
    if total_cell_count < 0 {
        return Err(XferStatus::InvalidData);
    }
    if let Some(grid) = snapshot.grid.as_ref() {
        if grid.cells.len() as i32 != total_cell_count {
            return Err(XferStatus::InvalidData);
        }
    }

    if xfer.get_xfer_mode() == XferMode::Load {
        let mut cells = vec![
            crate::system::shroud_manager::ShroudCellSnapshot::default();
            total_cell_count as usize
        ];
        for cell in &mut cells {
            xfer_partition_cell_shroud(xfer, cell)?;
        }
        if let Some(grid) = snapshot.grid.as_mut() {
            grid.cell_size = cell_size;
            grid.cells = cells;
        } else if total_cell_count > 0 {
            snapshot.grid = Some(crate::system::shroud_manager::ShroudGridSnapshot {
                width: total_cell_count as u32,
                height: 1,
                cell_size,
                cells,
            });
        }
    } else if let Some(grid) = snapshot.grid.as_mut() {
        for cell in &mut grid.cells {
            xfer_partition_cell_shroud(xfer, cell)?;
        }
    }

    if xfer.get_xfer_mode() == XferMode::Load {
        logic.partition_manager.cell_size = cell_size;
    }

    if version >= 2 {
        let mut queue_size = snapshot.pending_undo_shroud_reveals.len() as i32;
        xfer.xfer_int(&mut queue_size)?;
        if queue_size < 0 {
            return Err(XferStatus::InvalidData);
        }
        if xfer.get_xfer_mode() == XferMode::Load {
            snapshot.pending_undo_shroud_reveals = vec![
                crate::system::shroud_manager::ShroudPendingUndoRevealSnapshot::default();
                queue_size as usize
            ];
        }
        for info in &mut snapshot.pending_undo_shroud_reveals {
            xfer_sighting_info(xfer, info)?;
        }
    }

    if xfer.get_xfer_mode() == XferMode::Load {
        shroud
            .replace_state(&snapshot)
            .map_err(|_| XferStatus::InvalidData)?;
        shroud.refresh_shroud_for_local_player();
    }

    Ok(())
}




fn xfer_sides_list_runtime_state(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let sides = get_sides_list();
    let mut sides_guard = sides.write().map_err(|_| XferStatus::InvalidData)?;
    let mut side_count = sides_guard.get_num_sides() as i32;
    xfer.xfer_int(&mut side_count)?;
    if side_count != sides_guard.get_num_sides() as i32 {
        return Err(XferStatus::InvalidData);
    }

    for idx in 0..side_count.max(0) as usize {
        let side = sides_guard
            .get_side_info_mut(idx)
            .ok_or(XferStatus::InvalidData)?;
        let mut script_list_present = side.get_script_list().is_some();
        xfer.xfer_bool(&mut script_list_present)?;
        let has_runtime_script_list = side.get_script_list().is_some();
        if script_list_present != has_runtime_script_list {
            return Err(XferStatus::InvalidData);
        }
    }

    Ok(())
}

fn xfer_terrain_logic_runtime_state(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    let current_version: XferVersion = 2;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let terrain = get_terrain_logic();
    let mut terrain_guard = terrain.write().map_err(|_| XferStatus::InvalidData)?;

    let mut active_boundary = terrain_guard.get_active_boundary();
    xfer.xfer_int(&mut active_boundary)?;
    if xfer.get_xfer_mode() == XferMode::Load {
        terrain_guard.set_active_boundary(active_boundary);
    }

    if version >= 2 {
        let mut entries = if xfer.get_xfer_mode() == XferMode::Load {
            Vec::new()
        } else {
            terrain_guard.snapshot_dynamic_water_entries()
        };

        let mut entry_count = if xfer.get_xfer_mode() == XferMode::Load {
            0i32
        } else {
            entries.len() as i32
        };
        xfer.xfer_int(&mut entry_count)?;
        if entry_count < 0 {
            return Err(XferStatus::InvalidData);
        }

        if xfer.get_xfer_mode() == XferMode::Load {
            entries.reserve(entry_count as usize);
            for _ in 0..entry_count {
                let mut trigger_id: Int = -1;
                xfer.xfer_int(&mut trigger_id)?;
                let mut change_per_frame = 0.0f32;
                let mut target_height = 0.0f32;
                let mut damage_amount = 0.0f32;
                let mut current_height = 0.0f32;
                xfer.xfer_real(&mut change_per_frame)?;
                xfer.xfer_real(&mut target_height)?;
                xfer.xfer_real(&mut damage_amount)?;
                xfer.xfer_real(&mut current_height)?;
                entries.push(TerrainDynamicWaterSnapshotEntry {
                    trigger_id,
                    water_name: AsciiString::new(),
                    change_per_frame,
                    target_height,
                    damage_amount,
                    current_height,
                });
            }
            terrain_guard
                .restore_dynamic_water_entries(entries)
                .map_err(|_| XferStatus::InvalidData)?;
        } else {
            for entry in &mut entries {
                let mut trigger_id = entry.trigger_id;
                xfer.xfer_int(&mut trigger_id)?;
                xfer.xfer_real(&mut entry.change_per_frame)?;
                xfer.xfer_real(&mut entry.target_height)?;
                xfer.xfer_real(&mut entry.damage_amount)?;
                xfer.xfer_real(&mut entry.current_height)?;
            }
        }
    }

    Ok(())
}
