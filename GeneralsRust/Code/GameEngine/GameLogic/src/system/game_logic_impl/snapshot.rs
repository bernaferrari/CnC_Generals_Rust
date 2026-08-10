struct GameLogicSnapshotBridge {
    logic: &'static Mutex<GameLogic>,
}

impl GameLogicSnapshotBridge {
    fn new(logic: &'static Mutex<GameLogic>) -> Self {
        Self { logic }
    }
}

impl XferSnapshotTrait for GameLogicSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let mut guard = self.logic.lock().map_err(|_| XferStatus::InvalidData)?;
        xfer_game_logic_state(&mut guard, xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        let mut guard = self.logic.lock().map_err(|_| XferStatus::InvalidData)?;
        guard.load_post_process();
        Ok(())
    }
}

struct ScriptEngineSnapshotBridge {
    script_engine: Arc<RwLock<Option<ScriptEngine>>>,
}

impl ScriptEngineSnapshotBridge {
    fn new(script_engine: Arc<RwLock<Option<ScriptEngine>>>) -> Self {
        Self { script_engine }
    }

    fn with_engine_mut<R, F>(&self, mut callback: F) -> Result<R, XferStatus>
    where
        F: FnMut(&mut ScriptEngine) -> Result<R, XferStatus>,
    {
        let mut guard = self
            .script_engine
            .write()
            .map_err(|_| XferStatus::InvalidData)?;
        if guard.is_none() {
            *guard = Some(ScriptEngine::new().map_err(|_| XferStatus::InvalidData)?);
        }
        let engine = guard.as_mut().ok_or(XferStatus::InvalidData)?;
        callback(engine)
    }

    fn difficulty_to_i32(difficulty: GameDifficulty) -> i32 {
        match difficulty {
            GameDifficulty::Easy => 0,
            GameDifficulty::Normal => 1,
            GameDifficulty::Hard => 2,
            GameDifficulty::Brutal => 3,
        }
    }

    fn difficulty_from_i32(value: i32) -> GameDifficulty {
        match value {
            0 => GameDifficulty::Easy,
            2 => GameDifficulty::Hard,
            3 => GameDifficulty::Brutal,
            _ => GameDifficulty::Normal,
        }
    }

    fn xfer_script_engine_state(
        engine: &mut ScriptEngine,
        xfer: &mut dyn Xfer,
    ) -> Result<(), XferStatus> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        let mut frame_object_count_changed = engine.get_frame_object_count_changed();
        xfer.xfer_unsigned_int(&mut frame_object_count_changed)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            engine.set_frame_object_count_changed(frame_object_count_changed);
        }

        let mut shown_local_defeat_window = engine.has_shown_mp_local_defeat_window();
        xfer.xfer_bool(&mut shown_local_defeat_window)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            engine.set_shown_mp_local_defeat_window(shown_local_defeat_window);
        }

        let mut freeze_script = engine.is_time_frozen_script();
        xfer.xfer_bool(&mut freeze_script)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            if freeze_script {
                engine.do_freeze_time();
            } else {
                engine.do_unfreeze_time();
            }
        }

        let mut freeze_debug = engine.is_time_frozen_debug();
        xfer.xfer_bool(&mut freeze_debug)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            engine.set_time_frozen_debug(freeze_debug);
        }

        let mut current_track = engine.get_current_track_name().to_string();
        xfer.xfer_string(&mut current_track)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            engine.set_current_track_name(current_track);
        }

        let mut global_difficulty = Self::difficulty_to_i32(engine.get_global_difficulty());
        xfer.xfer_int(&mut global_difficulty)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            engine.set_global_difficulty(Self::difficulty_from_i32(global_difficulty));
        }

        let mut choose_victim_normal = engine.get_choose_victim_always_uses_normal();
        xfer.xfer_bool(&mut choose_victim_normal)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            engine.set_choose_victim_always_uses_normal(choose_victim_normal);
        }

        Ok(())
    }
}

impl XferSnapshotTrait for ScriptEngineSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.with_engine_mut(|engine| Self::xfer_script_engine_state(engine, xfer))
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

struct PartitionSnapshotBridge {
    logic: &'static Mutex<GameLogic>,
}

impl PartitionSnapshotBridge {
    fn new(logic: &'static Mutex<GameLogic>) -> Self {
        Self { logic }
    }
}

impl XferSnapshotTrait for PartitionSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let mut guard = self.logic.lock().map_err(|_| XferStatus::InvalidData)?;
        xfer_partition_state(&mut guard, xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

struct GhostObjectSnapshotBridge {
    manager: Arc<RwLock<crate::object::GhostObjectManager>>,
}

impl GhostObjectSnapshotBridge {
    fn new(manager: Arc<RwLock<crate::object::GhostObjectManager>>) -> Self {
        Self { manager }
    }
}

impl XferSnapshotTrait for GhostObjectSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let mut guard = self.manager.write().map_err(|_| XferStatus::InvalidData)?;
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        let mut local_player = guard.get_local_player_index();
        xfer.xfer_int(&mut local_player)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            guard.set_local_player_index(local_player);
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

struct PlayerListSnapshotBridge;

impl XferSnapshotTrait for PlayerListSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        xfer_player_list_runtime_state(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

struct TeamFactorySnapshotBridge;

impl XferSnapshotTrait for TeamFactorySnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        xfer_team_factory_runtime_state(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

struct SidesListSnapshotBridge;

impl XferSnapshotTrait for SidesListSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        xfer_sides_list_runtime_state(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

struct TerrainLogicSnapshotBridge;

impl XferSnapshotTrait for TerrainLogicSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        xfer_terrain_logic_runtime_state(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

fn register_game_logic_snapshot_block() {
    let logic = GAME_LOGIC.get_or_init(|| Mutex::new(GameLogic::default()));
    let script_engine = get_script_engine();
    let mut state = game_engine::System::get_game_state();
    state.add_snapshot_block(
        "CHUNK_TerrainLogic".to_string(),
        Box::new(TerrainLogicSnapshotBridge),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_TeamFactory".to_string(),
        Box::new(TeamFactorySnapshotBridge),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_TeamFactory".to_string(),
        Box::new(TeamFactorySnapshotBridge),
        game_engine::System::SnapshotType::DeepCrcLogicOnly,
    );
    state.add_snapshot_block(
        "CHUNK_Players".to_string(),
        Box::new(PlayerListSnapshotBridge),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_Players".to_string(),
        Box::new(PlayerListSnapshotBridge),
        game_engine::System::SnapshotType::DeepCrcLogicOnly,
    );
    state.add_snapshot_block(
        "CHUNK_GameLogic".to_string(),
        Box::new(GameLogicSnapshotBridge::new(logic)),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_GameLogic".to_string(),
        Box::new(GameLogicSnapshotBridge::new(logic)),
        game_engine::System::SnapshotType::DeepCrcLogicOnly,
    );
    state.add_snapshot_block(
        "CHUNK_ScriptEngine".to_string(),
        Box::new(ScriptEngineSnapshotBridge::new(script_engine.clone())),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_ScriptEngine".to_string(),
        Box::new(ScriptEngineSnapshotBridge::new(script_engine)),
        game_engine::System::SnapshotType::DeepCrcLogicOnly,
    );
    state.add_snapshot_block(
        "CHUNK_SidesList".to_string(),
        Box::new(SidesListSnapshotBridge),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_SidesList".to_string(),
        Box::new(SidesListSnapshotBridge),
        game_engine::System::SnapshotType::DeepCrcLogicOnly,
    );
    state.add_snapshot_block(
        "CHUNK_Partition".to_string(),
        Box::new(PartitionSnapshotBridge::new(logic)),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_Partition".to_string(),
        Box::new(PartitionSnapshotBridge::new(logic)),
        game_engine::System::SnapshotType::DeepCrcLogicOnly,
    );
    state.add_snapshot_block(
        "CHUNK_GhostObject".to_string(),
        Box::new(GhostObjectSnapshotBridge::new(Arc::clone(
            &THE_GHOST_OBJECT_MANAGER,
        ))),
        game_engine::System::SnapshotType::SaveLoad,
    );
}
