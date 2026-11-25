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
        // C++ GameLogic::crc is empty. DeepCrc must not emit the v10 save blob.
        let _ = xfer;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let mut guard = self.logic.lock().map_err(|_| XferStatus::InvalidData)?;
        guard.xfer_native_snapshot(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        let mut guard = self.logic.lock().map_err(|_| XferStatus::InvalidData)?;
        guard.load_post_process()
    }
}

struct ScriptEngineSnapshotBridge {
    script_engine: crate::scripting::engine::ScriptEngineHandle,
    /// The registering world; an empty slot is rebuilt on it, never on
    /// whichever world is published when the save runs.
    world: std::sync::Weak<crate::system::engine_stores::WorldServices>,
}

impl ScriptEngineSnapshotBridge {
    fn new(world: &std::sync::Arc<crate::system::engine_stores::WorldServices>) -> Self {
        Self {
            script_engine: world.scripts().engine().clone(),
            world: std::sync::Arc::downgrade(world),
        }
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
            let world = self.world.upgrade().ok_or(XferStatus::InvalidData)?;
            *guard = Some(ScriptEngine::new_with_services(&world).map_err(|_| XferStatus::InvalidData)?);
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

        let mut current_track = engine.get_current_track_name();
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

struct PlayerListSnapshotBridge(Arc<RwLock<crate::player::PlayerList>>);

impl XferSnapshotTrait for PlayerListSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        xfer_player_list_crc(&self.0, xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        xfer_player_list_runtime_state(&self.0, xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        use game_engine::common::system::snapshot::Snapshotable;

        let players: Vec<_> = self
            .0
            .read()
            .map_err(|_| XferStatus::InvalidData)?
            .iter()
            .cloned()
            .collect();
        for player_arc in players {
            let mut player = player_arc.write().map_err(|_| XferStatus::InvalidData)?;
            Snapshotable::load_post_process(&mut *player).map_err(|_| XferStatus::InvalidData)?;
        }
        Ok(())
    }
}

struct TeamFactorySnapshotBridge(crate::team::TeamFactoryHandle);

impl XferSnapshotTrait for TeamFactorySnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        xfer_team_factory_crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let mut factory = self.0.lock().map_err(|_| XferStatus::InvalidData)?;
        xfer_team_factory_runtime_state(&mut factory, xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        let mut factory = self.0.lock().map_err(|_| XferStatus::InvalidData)?;
        factory.restore_unique_ids_after_load();
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

struct TerrainLogicSnapshotBridge {
    logic: &'static Mutex<GameLogic>,
    terrain: Arc<RwLock<crate::terrain::TerrainLogic>>,
}

impl XferSnapshotTrait for TerrainLogicSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        xfer_terrain_logic_runtime_state(self.logic, &self.terrain, xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        let logic = self.logic.try_lock().map_err(|_| XferStatus::InvalidData)?;
        let pathfinder = logic.pathfinding_owner.as_ref()
            .and_then(|owner| owner.ai().read().ok()?.pathfinder());
        let mut pathfinder = pathfinder.as_ref()
            .map(|pathfinder| pathfinder.try_write().map_err(|_| XferStatus::InvalidData))
            .transpose()?;
        let removed = self.terrain.write().map_err(|_| XferStatus::InvalidData)?
            .remove_orphaned_bridges(|id| logic.objects.contains_key(&id));
        if let Some(pathfinder) = pathfinder.as_deref_mut() {
            for layer in removed {
                pathfinder.change_bridge_state(layer, false);
            }
        }
        Ok(())
    }
}

fn register_game_logic_snapshot_block() {
    // Direct Core slot: must not call get_game_logic() here — that re-enters
    // SAVE_GAME_COUNTER_HOOKS OnceLock while install_save_game_counter runs.
    // The registering world owns both the logic and the script engine the
    // bridges serialize (C++ xferSnapshot(TheScriptEngine) of that world).
    let (logic, world) = if let Some(core) =
        crate::system::engine_stores::published_core_for_game_logic()
    {
        (core.game_logic_mutex(), std::sync::Arc::clone(core.services()))
    } else if let Some(services) = crate::system::engine_stores::try_active_services() {
        (services.game_logic_mutex(), services)
    } else {
        panic!("snapshot block registration requires published Core or WorldServices");
    };
    let mut state = game_engine::System::get_game_state();
    state.add_snapshot_block(
        "CHUNK_TerrainLogic".to_string(),
        Box::new(TerrainLogicSnapshotBridge {
            logic,
            terrain: Arc::clone(world.terrain()),
        }),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_TeamFactory".to_string(),
        Box::new(TeamFactorySnapshotBridge(world.team_factory().clone())),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_TeamFactory".to_string(),
        Box::new(TeamFactorySnapshotBridge(world.team_factory().clone())),
        game_engine::System::SnapshotType::DeepCrcLogicOnly,
    );
    state.add_snapshot_block(
        "CHUNK_Players".to_string(),
        Box::new(PlayerListSnapshotBridge(Arc::clone(world.players()))),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_Players".to_string(),
        Box::new(PlayerListSnapshotBridge(Arc::clone(world.players()))),
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
        Box::new(ScriptEngineSnapshotBridge::new(&world)),
        game_engine::System::SnapshotType::SaveLoad,
    );
    state.add_snapshot_block(
        "CHUNK_ScriptEngine".to_string(),
        Box::new(ScriptEngineSnapshotBridge::new(&world)),
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
            &the_ghost_object_manager(),
        ))),
        game_engine::System::SnapshotType::SaveLoad,
    );
}

/// Per-operation Core snapshot adapter with a serializer-owned campaign DTO.
pub struct CampaignGameLogicSnapshot<'a> {
    pub logic: &'a mut GameLogic,
    pub campaign: &'a mut game_engine::System::CampaignManagerXferState,
}
impl XferSnapshotTrait for CampaignGameLogicSnapshot<'_> {
    fn saved_map_runtime(&mut self) -> Option<&mut dyn SavedMapRuntime> {
        Some(self.logic)
    }

    fn crc(&mut self, _xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        Ok(())
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.logic
            .xfer_native_snapshot_with_campaign(xfer, self.campaign)
    }
    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        self.logic.load_post_process()
    }
}
