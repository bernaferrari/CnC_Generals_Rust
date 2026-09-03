#[cfg(test)]
thread_local! {
    /// Test-only: queue this id from inside processDestroyList, matching C++
    /// "the new object was added to the end of this list" (GameLogic.cpp:2509).
    static CLEANUP_CASCADE_CHILD: std::cell::Cell<Option<ObjectID>> =
        const { std::cell::Cell::new(None) };
}

impl GameLogic {
    pub fn cleanup_dead_objects(&mut self) -> Result<(), GameLogicError> {
        // Wave 344: empty dual-world → Ok(()). Still clean live dead-list.
        if dual_world_registry_unavailable() && self.dead_objects.is_empty() {
            return Ok(());
        }

        trace!(
            "GameLogic::cleanup_dead_objects() - {} objects to clean",
            self.dead_objects.len()
        );

        // Track if we processed any objects for FOW updates
        let had_dead_objects = !self.dead_objects.is_empty();

        // C++ GameLogic::processDestroyList (GameLogic.cpp:2449-2510) walks
        // `iterator != end()` so objects queued during deletion (sub-objects)
        // are processed the same frame. Do not drain the list up-front.
        let mut i = 0;
        while i < self.dead_objects.len() {
            let obj_id = self.dead_objects[i];
            i += 1;
            #[cfg(test)]
            if let Some(child) = CLEANUP_CASCADE_CHILD.with(|c| c.take()) {
                self.destroy_object(child);
            }
            let mut object_position = None;
            let object_index = self.all_objects.iter().position(|&id| id == obj_id);
            let previous_object_id = object_index
                .and_then(|index| index.checked_sub(1))
                .and_then(|index| self.all_objects.get(index).copied());
            let next_object_id =
                object_index.and_then(|index| self.all_objects.get(index + 1).copied());

            if let Some(previous_id) = previous_object_id {
                if let Some(previous_object) = self.objects.get(&previous_id) {
                    if let Ok(mut previous_guard) = previous_object.write() {
                        previous_guard.set_next_object_id(next_object_id);
                    }
                }
            }
            if let Some(next_id) = next_object_id {
                if let Some(next_object) = self.objects.get(&next_id) {
                    if let Ok(mut next_guard) = next_object.write() {
                        next_guard.set_prev_object_id(previous_object_id);
                    }
                }
            }

            if let Some(obj_ref) = self.objects.remove(&obj_id) {
                if let Ok(obj_read) = obj_ref.read() {
                    object_position = Some(*obj_read.get_position());
                }

                if let Ok(mut obj_write) = obj_ref.write() {
                    // C++ Object::onDestroy already ran in destroyObject.
                    // If this id was queued without destroyObject, finish
                    // contain-eject / module onDelete / partition here.
                    if !obj_write.is_destroyed() {
                        obj_write.on_destroy();
                    }
                    obj_write.set_next_object_id(None);
                    obj_write.set_prev_object_id(None);
                }

                // Remove all update-module registrations for this object regardless of
                // whether it used `on_destroy()` prior to cleanup.
                self.remove_updates_for_object(obj_id);

                // Keep the script named-object cache in sync (C++ ScriptEngine::addObjectToCache parity).
                // Safe to call even if the object was never registered or had no name.
                let _ =
                    crate::scripting::engine::get_named_object_tracker().unregister_object(obj_id);
            }

            // Remove from object list
            self.all_objects.retain(|&id| id != obj_id);

            // Remove from objects map
            if let Some(pos) = object_position {
                let _ = with_ai_integration_mut(|manager| {
                    let _ = manager.notify_object_destroyed(obj_id, &[pos]);
                });
            }

            // Fire destruction event
            self.event_queue.push(GameEvent::ObjectDestroyed(obj_id));

            // Remove from partition manager
            if let Ok(mut ghost_manager) = crate::object::THE_W3D_GHOST_OBJECT_MANAGER.write() {
                self.partition_manager
                    .detach_object_ghost(obj_id, &mut ghost_manager);
            }
            self.partition_manager.remove_object(obj_id);

            let _ = with_collision_system_mut(|system| {
                let _ = system.unregister_object(obj_id);
                Ok::<(), crate::object::collide::CollisionError>(())
            });

            // In full implementation:
            // - Release contained objects
            // - Remove from team/group
            // - Award experience to killer
            // - Spawn death effects

            // Unregister from global registry
            OBJECT_REGISTRY.unregister_object(obj_id);

            trace!("Destroyed object {}", obj_id);
        }
        self.dead_objects.clear();

        // Trigger FOW update if any objects were destroyed
        if had_dead_objects {
            if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
                shroud_mgr.force_update();
            }
        }

        Ok(())
    }

    pub fn register_object(
        &mut self,
        object: Arc<RwLock<Object>>,
    ) -> Result<ObjectID, GameLogicError> {
        let (object_id, object_name) = {
            let guard = object
                .read()
                .map_err(|_| GameLogicError::Generic("Object lock poisoned".to_string()))?;
            (guard.get_id(), guard.get_name().to_string())
        };

        if object_id == INVALID_ID {
            return Err(GameLogicError::InvalidState(
                "Attempted to register object without valid ID".to_string(),
            ));
        }

        // C++ GameLogic::registerObject prepends (GameLogic.cpp:3866).
        self.objects.insert(object_id, Arc::clone(&object));
        self.prepend_to_object_list(&object, object_id);

        // Register in global registry
        OBJECT_REGISTRY.register_object(object_id, &object);

        // Register in the scripting named-object cache (C++ ScriptEngine::addObjectToCache).
        if !object_name.is_empty() {
            let tracker = crate::scripting::engine::get_named_object_tracker();
            let _ = tracker.register_named_object(object_name, object_id);
        }

        // C++ Object::initObject → sendObjectCreated after the object exists.
        // Dual-world: bind a client drawable when the logic object has none.
        send_object_created(&object);

        // Add to partition manager
        if let Ok(obj) = object.read() {
            let pos = obj.get_position();
            self.partition_manager
                .add_object(object_id, (pos.x, pos.y, pos.z));
            let ghost_eligible = obj.is_kind_of(KindOf::Immobile)
                && !obj
                    .get_template()
                    .get_draw_module_info()
                    .iter()
                    .any(|module| module.name.as_str().eq_ignore_ascii_case("W3DDefaultDraw"));
            if let Ok(mut ghost_manager) = crate::object::THE_W3D_GHOST_OBJECT_MANAGER.write() {
                self.partition_manager.attach_object_ghost(
                    object_id,
                    ghost_eligible,
                    &mut ghost_manager,
                );
            }

            let geom =
                map_collision_geometry(&obj.get_geometry_info(), obj.get_template_geometry_type());
            let _ = with_collision_system_mut(|system| {
                let _ = system.register_object(
                    object_id,
                    crate::object::collide::Coord3D::new(pos.x, pos.y, pos.z),
                    geom,
                    None,
                );
                let cfg = if obj.is_kind_of(KindOf::Projectile) {
                    CollisionResponseConfig {
                        response_type: CollisionResponseType::None,
                        ..Default::default()
                    }
                } else if obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Building)
                    || obj.is_kind_of(KindOf::Bridge)
                    || obj.is_kind_of(KindOf::Barrier)
                {
                    CollisionResponseConfig::blocking()
                } else {
                    CollisionResponseConfig::default()
                };
                system.set_collision_config(object_id, cfg);
                Ok::<(), crate::object::collide::CollisionError>(())
            });

            let is_ai_controlled = obj.get_ai_update_interface().is_some();
            let is_obstacle = obj.is_kind_of(KindOf::Building)
                || obj.is_kind_of(KindOf::Structure)
                || obj.is_kind_of(KindOf::Bridge)
                || obj.is_kind_of(KindOf::Barrier);
            let _ = with_ai_integration_mut(|manager| {
                let _ =
                    manager.notify_object_created(object_id, *pos, is_ai_controlled, is_obstacle);
            });
        }

        // Fire creation event
        self.event_queue.push(GameEvent::ObjectCreated(object_id));

        // Trigger FOW update for new object
        // New objects create new vision sources, so FOW needs to be recalculated
        if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
            shroud_mgr.force_update();
        }

        debug!("Registered object {}", object_id);
        Ok(object_id)
    }

    /// Add an already-registered object to the internal update lists
    /// (`all_objects` / `objects`) without re-registering in OBJECT_REGISTRY.
    ///
    /// This is used when an object was registered via `TheGameLogic::register_object()`
    /// (which only touches OBJECT_REGISTRY) but also needs to be visible to
    /// `process_object_updates()`.
    pub fn track_object_in_update_list(
        &mut self,
        object: Arc<RwLock<Object>>,
    ) -> Result<ObjectID, GameLogicError> {
        let object_id = {
            let guard = object
                .read()
                .map_err(|_| GameLogicError::Generic("Object lock poisoned".to_string()))?;
            guard.get_id()
        };

        if object_id == INVALID_ID {
            return Err(GameLogicError::InvalidState(
                "Attempted to track object without valid ID".to_string(),
            ));
        }

        if self.objects.contains_key(&object_id) {
            return Ok(object_id);
        }

        // C++ registerObject prepends (GameLogic.cpp:3866).
        self.objects.insert(object_id, Arc::clone(&object));
        self.prepend_to_object_list(&object, object_id);

        Ok(object_id)
    }

    /// Mark an object for destruction
    ///
    /// ## C++ Reference: GameLogic::destroyObject() (GameLogic.cpp)
    pub fn destroy_object(&mut self, object_id: ObjectID) {
        if object_id == INVALID_ID {
            return;
        }

        if let Some(obj_arc) = self.objects.get(&object_id).cloned() {
            if let Ok(mut obj) = obj_arc.write() {
                if obj.is_destroyed() {
                    return;
                }
                // C++ DestroyModuleInterface::onDestroy before status bit.
                let behaviors = obj.get_behavior_modules();
                for behavior in behaviors {
                    if let Ok(mut module) = behavior.lock() {
                        if let Some(destroy) = module.get_destroy() {
                            destroy.on_destroy(object_id);
                        }
                    }
                }
                // C++ immediately sets OBJECT_STATUS_DESTROYED so same-frame
                // isDestroyed() checks stop firing/pathing.
                obj.set_status(ObjectStatusTypes::Destroyed.into(), true);
                if let Some(ai) = obj.get_ai_update_interface() {
                    if let Ok(mut ai) = ai.lock() {
                        ai.set_locomotor_goal_none();
                        ai.destroy_path();
                    }
                }
            }
        }

        // Queue for destruction at end of frame
        if !self.dead_objects.contains(&object_id) {
            self.dead_objects.push(object_id);
            debug!("Queued object {} for destruction", object_id);
        }

        // C++ GameLogic::destroyObject (GameLogic.cpp:3966-3980):
        // obj->onDestroy() then remove WALK_ON_TOP_OF_WALL from the pathfinder
        // and mark the control bar dirty for local special-power objects.
        // Do not split onDestroy across destroyObject / processDestroyList.
        let (is_wall, has_special_power, is_local) =
            if let Some(obj_arc) = self.objects.get(&object_id).cloned() {
                if let Ok(mut obj) = obj_arc.write() {
                    let is_wall = obj.is_kind_of(KindOf::WalkOnTopOfWall);
                    let has_special_power = obj.has_any_special_power();
                    let is_local = obj.is_locally_controlled();
                    obj.on_destroy();
                    (is_wall, has_special_power, is_local)
                } else {
                    (false, false, false)
                }
            } else {
                (false, false, false)
            };

        if is_wall {
            let ai_store = the_ai(); if let Ok(ai) = ai_store.read() {
                if let Some(pf) = ai.pathfinder() {
                    if let Ok(mut pf) = pf.write() {
                        pf.remove_wall_piece(object_id);
                    }
                }
            }
        }
        if has_special_power && is_local {
            crate::control_bar::mark_ui_dirty();
        }
    }

    /// C++ `Object::prependToList(&m_objList)` — newest object is list head.
    fn prepend_to_object_list(&mut self, object: &Arc<RwLock<Object>>, object_id: ObjectID) {
        let old_head = self.all_objects.first().copied();
        self.all_objects.insert(0, object_id);
        if let Ok(mut object_guard) = object.write() {
            object_guard.set_prev_object_id(None);
            object_guard.set_next_object_id(old_head);
        }
        if let Some(old_id) = old_head {
            if let Some(old_object) = self.objects.get(&old_id) {
                if let Ok(mut old_guard) = old_object.write() {
                    old_guard.set_prev_object_id(Some(object_id));
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn test_queue_cleanup_cascade(child: ObjectID) {
        CLEANUP_CASCADE_CHILD.with(|c| c.set(Some(child)));
    }

    /// Find an object by its ID
    ///
    /// ## C++ Reference: GameLogic::findObjectByID() (GameLogic.h inline)
    pub fn find_object_by_id(&self, object_id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        self.objects.get(&object_id).cloned()
    }

    /// Allocate a unique object ID
    ///
    /// ## C++ Reference: GameLogic::allocateObjectID() (GameLogic.cpp)
    pub fn allocate_object_id(&mut self) -> ObjectID {
        let id = self.next_object_id;
        self.next_object_id = self.next_object_id.wrapping_add(1);
        if self.next_object_id == INVALID_ID {
            self.next_object_id = 1;
        }
        id
    }

    /// Get the next object-id counter value (C++ GameLogic::getObjectIDCounter).
    pub fn get_object_id_counter(&self) -> ObjectID {
        self.next_object_id
    }

    /// Set the next object-id counter value (C++ GameLogic::setObjectIDCounter).
    pub fn set_object_id_counter(&mut self, next_object_id: ObjectID) {
        let normalized = if next_object_id == 0 || next_object_id == INVALID_ID {
            1
        } else {
            next_object_id
        };
        self.next_object_id = normalized;
    }

    /// Get the first object (for iteration)
    pub fn get_first_object_id(&self) -> Option<ObjectID> {
        self.all_objects.first().copied()
    }

    pub fn get_first_object(&self) -> Option<Arc<RwLock<Object>>> {
        self.get_first_object_id()
            .and_then(|id| self.objects.get(&id).cloned())
    }

    /// Stable object IDs in list order (no Arc retention).
    pub fn get_all_object_ids(&self) -> &[ObjectID] {
        &self.all_objects
    }

    /// Get object count
    pub fn get_object_count(&self) -> usize {
        self.all_objects.len()
    }

    /// Get current number of queued sleepy update modules.
    ///

    pub fn get_frame(&self) -> UnsignedInt {
        self.frame
    }

    pub fn get_game_time(&self) -> f32 {
        self.game_time
    }

    pub fn is_in_game_logic_update(&self) -> Bool {
        self.is_in_update
    }

    pub fn set_dimensions(&mut self, width: Real, height: Real) {
        self.width = width;
        self.height = height;
    }

    pub fn get_width(&self) -> Real {
        self.width
    }

    pub fn get_height(&self) -> Real {
        self.height
    }

    pub fn set_game_mode(&mut self, mode: Int) {
        self.game_mode = mode;
    }

    pub fn get_game_mode(&self) -> Int {
        self.game_mode
    }

    pub fn is_in_single_player_game(&self) -> Bool {
        self.game_mode == GAME_SINGLE_PLAYER
    }

    pub fn is_in_multiplayer_game(&self) -> Bool {
        self.game_mode == GAME_LAN || self.game_mode == GAME_INTERNET
    }

    pub fn is_in_skirmish_game(&self) -> Bool {
        self.game_mode == GAME_SKIRMISH
    }

    pub fn set_loading_map(&mut self, loading: Bool) {
        self.loading_map = loading;
    }

    pub fn is_loading_map(&self) -> Bool {
        self.loading_map || crate::helpers::TheGameLogic::is_start_new_game_requested()
    }

    /// Complete the heavy new-game initialization path.
    ///
    /// The request is staged first, then this method performs the actual map
    /// load once the movie gate allows it.
    pub(crate) fn start_new_game_now(&mut self, loading_save_game: Bool) -> Result<(), String> {
        let map_path = game_engine::common::ini::get_global_data()
            .map(|data| data.read().map_name.clone())
            .unwrap_or_default();

        if map_path.is_empty() {
            crate::helpers::TheGameLogic::clear_start_new_game_request();
            return Err("Cannot start game: global map_name is empty".to_string());
        }


        // C++ GameLogic.cpp:1254-1256 — always clear campaign win before map load.
        clear_campaign_victorious_for_new_game();

        if !loading_save_game {
            let mut state = game_engine::System::get_game_state();
            state.set_pristine_map_name(map_path.clone());
            if state.is_in_save_directory(std::path::Path::new(&map_path)) {
                log::error!(
                    "Pristine map name points to save directory map '{}'; sidecar lookup may diverge from C++ expected source-map semantics",
                    map_path
                );
            }
        }

        // Match C++ startNewGame(): the transition re-applies FP mode and clears the
        // staged start request before the actual map load begins.
        set_fp_mode();
        self.set_loading_map(true);
        crate::helpers::TheGameLogic::clear_start_new_game_request();
        let game_mode_int = self.get_game_mode();
        crate::helpers::TheGameLogic::begin_load_screen(game_mode_int, loading_save_game);
        crate::helpers::TheGameLogic::update_load_progress(
            crate::system::game_initialization::LOAD_PROGRESS_START,
        );

        let game_mode = match game_mode_int {
            GAME_SHELL => crate::system::game_initialization::GameMode::ShellMap,
            GAME_SKIRMISH => crate::system::game_initialization::GameMode::Skirmish,
            GAME_LAN | GAME_INTERNET => crate::system::game_initialization::GameMode::Multiplayer,
            GAME_REPLAY => crate::system::game_initialization::GameMode::Replay,
            _ => crate::system::game_initialization::GameMode::SinglePlayer,
        };

        let difficulty = match crate::helpers::TheScriptEngine::get_global_difficulty() {
            0 => crate::system::game_initialization::GameDifficulty::Easy,
            2 => crate::system::game_initialization::GameDifficulty::Hard,
            3 => crate::system::game_initialization::GameDifficulty::Brutal,
            _ => crate::system::game_initialization::GameDifficulty::Normal,
        };

        let num_players = if let Ok(sides_guard) = crate::sides_list::get_sides_list().read() {
            let count = sides_guard.get_num_sides().max(1) as usize;
            count.min(crate::system::player_init::MAX_PLAYER_COUNT)
        } else if let Ok(player_list) = crate::player::ThePlayerList().read() {
            let count = player_list.iter().count();
            if count > 0 {
                count.min(crate::system::player_init::MAX_PLAYER_COUNT)
            } else {
                2
            }
        } else {
            2
        };

        let params = crate::system::game_initialization::GameInitParams {
            map_path,
            game_mode,
            difficulty,
            num_players,
            player_templates: Vec::new(),
            victory_type: crate::system::victory_conditions::VictoryType::Annihilation,
            score_limit: None,
            time_limit: None,
            fog_of_war_enabled: true,
            starting_resources: crate::system::game_initialization::default_starting_cash(),
            ai_script: "DefaultAI".to_string(),
        };

        let init_result =
            crate::system::game_initialization::GameInitializer::initialize_game(params)
                .map(|_| ())
                .map_err(|err| format!("Game initialization failed: {}", err));
        if init_result.is_ok() {
            // C++ GameLogic.cpp:2073-2119 after PlayerList::newMap.
            apply_challenge_the_player_relationships();
            crate::helpers::TheGameLogic::update_load_progress(
                crate::system::game_initialization::LOAD_PROGRESS_END,
            );
            crate::helpers::TheGameLogic::run_load_screen_completion_transition(loading_save_game);
        }
        crate::helpers::TheGameLogic::end_load_screen();
        self.set_loading_map(false);
        // C++ GameLogic.cpp:2009-2010 — TheRecorder->initControls() after preload.
        game_engine::common::recorder::with_recorder_mut(|recorder| {
            recorder.init_controls();
        });
        // C++ GameLogic.cpp:2340-2343 replay start hint.
        if crate::helpers::TheGameLogic::is_in_replay_game() {
            crate::helpers::TheInGameUI::display_message("GUI:FastForwardInstructions");
        }
        init_result
    }

    pub fn set_loading_save(&mut self, loading: Bool) {
        self.loading_save = loading;
    }

    pub fn is_loading_save(&self) -> Bool {
        self.loading_save
    }

    /// Queue a command for processing
    pub fn queue_command(&mut self, command: GameCommand) {
        self.command_queue.push_back(command);
    }

    /// Queue damage for physics resolution
    pub fn queue_damage(&mut self, target: ObjectID, attacker: ObjectID, amount: f32) {
        self.physics_world.queue_damage(target, attacker, amount);
    }

    pub fn queue_objects_changed_trigger_areas(&mut self, object_id: ObjectID) {
        if object_id == INVALID_ID {
            return;
        }

        self.objects_changed_trigger_areas.push_back(object_id);
        self.frame_objects_changed_trigger_areas = self.frame;
        crate::ai::set_frame_objects_changed_trigger_areas(self.frame);
    }

    pub fn get_frame_objects_changed_trigger_areas(&self) -> UnsignedInt {
        self.frame_objects_changed_trigger_areas
    }


    pub fn update_objects_changed_trigger_areas(&mut self) {
        while let Some(object_id) = self.objects_changed_trigger_areas.pop_front() {
            trace!(
                "GameLogic::update_objects_changed_trigger_areas(object_id={})",
                object_id
            );
        }
    }

    /// Get object by ID (for command executor)
    pub fn get_object(&self, object_id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        self.objects.get(&object_id).cloned()
    }

    /// Get object handle by ID for mutation (callers must lock the returned handle)
    pub fn get_object_mut(&mut self, object_id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        self.get_object(object_id)
    }

    /// Get player by ID (for command executor)
    pub fn get_player(&self, player_id: u32) -> Option<Arc<RwLock<Player>>> {
        if let Ok(player_list_guard) = player_list().read() {
            for player_arc in player_list_guard.iter() {
                if let Ok(player) = player_arc.read() {
                    if player.get_player_index() == player_id as Int {
                        return Some(Arc::clone(player_arc));
                    }
                }
            }
        }
        None
    }

    /// Get mutable player by ID (for command executor)
    pub fn get_player_mut(&mut self, player_id: u32) -> Option<Arc<RwLock<Player>>> {
        self.get_player(player_id)
    }

    // =========================================================================
    // Snapshot/Save-Load Support Methods
    // =========================================================================

    /// Get current frame number (alias for get_frame for snapshot compatibility)
    /// Matches C++ GameLogic::getFrame
    pub fn get_current_frame(&self) -> u64 {
        self.frame as u64
    }

    /// Set current frame number (for restoring from snapshot)
    /// Matches C++ GameLogic::setFrame
    pub fn set_current_frame(&mut self, frame: u64) {
        self.frame = frame as UnsignedInt;
    }

    /// Get random seed for deterministic replay
    /// Matches C++ GameLogic::getRandomSeed
    pub fn get_random_seed(&self) -> u64 {
        self.random_seed
    }

    /// Set random seed (for restoring from snapshot)
    /// Matches C++ GameLogic::setRandomSeed
    pub fn set_random_seed(&mut self, seed: u64) {
        self.random_seed = seed;
    }

    /// Iterate over all objects in the game
    /// Returns iterator yielding Arc<RwLock<Object>> for each object
    pub fn iter_all_object_ids(&self) -> impl Iterator<Item = ObjectID> + '_ {
        self.all_objects.iter().copied()
    }

    pub fn iter_all_objects(&self) -> impl Iterator<Item = Arc<RwLock<Object>>> + '_ {
        self.objects.values().cloned()
    }

    /// Iterate over all players in the game
    /// Returns iterator yielding Arc<RwLock<Player>> for each player
    pub fn iter_players(&self) -> Vec<Arc<RwLock<Player>>> {
        let mut players = Vec::new();
        if let Ok(player_list_guard) = player_list().read() {
            for player_arc in player_list_guard.iter() {
                players.push(Arc::clone(player_arc));
            }
        }
        players
    }

    pub fn clear_all_objects(&mut self) {
        if let Ok(mut ghost_manager) = crate::object::THE_W3D_GHOST_OBJECT_MANAGER.write() {
            self.partition_manager
                .clear_ghost_objects(&mut ghost_manager);
        }
        // Clear update module tracking first
        self.sleepy_updates.clear();
        self.normal_updates.clear();
        self.module_lookup.clear();

        // Clear object lists
        self.all_objects.clear();
        self.dead_objects.clear();
        self.objects.clear();

        // Reset object ID counter
        self.next_object_id = 1;

        // Clear event and command queues
        self.event_queue.clear();
        self.command_queue.clear();
        self.radar_updates.clear();
        self.objects_changed_trigger_areas.clear();

        log::debug!("Cleared all objects from GameLogic");
    }

    /// Rebuild spatial partition index after loading
    /// Matches C++ GameLogic::rebuildPartitionManager
    pub fn rebuild_spatial_index(&mut self) {
        self.partition_manager.rebuild();
        log::debug!("Rebuilt spatial index");
    }

    /// Rebuild selection cache after loading
    /// This ensures UI selection state is consistent
    pub fn rebuild_selection_cache(&mut self) {
        // Selection cache is managed by GameClient; GameLogic has no additional state to rebuild.
        log::debug!("Selection cache rebuild requested");
    }

    /// Create an object from a template name for save/load restoration.
    /// Mirrors C++ GameLogic::createObjectFromTemplate for save-load rehydration.
    pub fn create_object_from_template(
        &mut self,
        template_name: &str,
        object_id: ObjectID,
    ) -> Result<Arc<RwLock<Object>>, GameLogicError> {
        let template =
            crate::helpers::TheThingFactory::find_template(template_name).ok_or_else(|| {
                GameLogicError::InvalidState(format!("Template not found: {}", template_name))
            })?;

        let id = if object_id == INVALID_ID {
            self.allocate_object_id()
        } else {
            if object_id >= self.next_object_id {
                self.next_object_id = object_id + 1;
            }
            object_id
        };

        let status_mask = ObjectStatusMaskType::none();
        let object = Object::new_with_id(template, id, status_mask, None)
            .map_err(|err| GameLogicError::Generic(err.to_string()))?;

        self.register_object(object.clone())?;

        Ok(object)
    }

    /// Add a restored object to the game world
    /// Used during save game loading
    pub fn add_restored_object(&mut self, object_arc: Arc<RwLock<Object>>) {
        let object_id = if let Ok(obj) = object_arc.read() {
            obj.get_id()
        } else {
            log::error!("Failed to read object for restoration");
            return;
        };

        // C++ restore lands on m_objList (prepend) and is findable.
        self.objects.insert(object_id, Arc::clone(&object_arc));
        self.prepend_to_object_list(&object_arc, object_id);
        OBJECT_REGISTRY.register_object(object_id, &object_arc);

        // Register with partition manager
        if let Ok(obj) = object_arc.read() {
            let pos = obj.get_position();
            self.partition_manager
                .register_object(object_id, pos.x, pos.y);
        }

        log::debug!("Added restored object with ID {}", object_id);
    }

    pub fn get_global_weapon_bonus_set(&self) -> &WeaponBonusSet {
        &self.global_weapon_bonus_set
    }

    // =========================================================================
    // C++ Parity: setDefaults, destroyAllObjectsImmediate, processDestroyList
    // =========================================================================

    /// PARITY_NOTE: GameLogic::setDefaults(Bool loadingSaveGame) C++ line 247.
    /// Resets frame counter, world dimensions, and update module lists.
    /// When `loading_save_game` is false, the object-ID allocator is also reset to 1.
    pub fn set_defaults(&mut self, loading_save_game: bool) {
        self.frame = 0;
        self.width = DEFAULT_WORLD_WIDTH;
        self.height = DEFAULT_WORLD_HEIGHT;
        self.normal_updates.clear();
        for _entry in &self.sleepy_updates {
            // C++: (*it)->friend_setIndexInLogic(-1)
        }
        self.sleepy_updates.clear();
        if !loading_save_game {
            self.next_object_id = 1;
        }
    }

    /// PARITY_NOTE: GameLogic::destroyAllObjectsImmediate() C++ line 285.
    /// Iterates all live objects, destroys every one, then immediately
    /// processes the destroy list. Used during `reset()`.
    pub fn destroy_all_objects_immediate(&mut self) {
        let all_ids: Vec<ObjectID> = self.all_objects.drain(..).collect();
        for obj_id in &all_ids {
            self.destroy_object(*obj_id);
        }
        let _ = self.cleanup_dead_objects();
        debug_assert!(
            self.all_objects.is_empty(),
            "destroyAllObjectsImmediate: object list not cleared"
        );
    }

    /// PARITY_NOTE: GameLogic::processDestroyList() C++ line 2445.
    /// C++ name alias for `cleanup_dead_objects`.
    pub fn process_destroy_list(&mut self) -> Result<(), GameLogicError> {
        self.cleanup_dead_objects()
    }

    // =========================================================================
    // C++ Parity: selectObject / deselectObject
    // =========================================================================

    /// PARITY_NOTE: GameLogic::selectObject(Object*, Bool, PlayerMaskType, Bool)
    /// C++ `GameLogic.cpp:2595-2641`:
    /// - reject null / non-mass-selectable unless `createNewSelection`
    /// - for each player in `playerMask`: `TheAI->createGroup()`, `group->add(obj)`,
    ///   then `setCurrentlySelectedAIGroup` or `addAIGroupToCurrentSelection`
    /// - if `affectClient`: `TheInGameUI->selectDrawable(obj->getDrawable())`
    ///
    /// Delegates to the live helper's player-assignment (`apply_select_object`).
    /// The `&Object` helper cannot run while this method holds the object
    /// `RwLock` — `AIGroup::add` try-reads the same Arc (Darwin deadlock).
    pub fn select_object(
        &mut self,
        object_id: ObjectID,
        create_new_selection: bool,
        player_mask: PlayerMaskType,
        affect_client: bool,
    ) {
        let Some(obj_ref) = self.find_object_by_id(object_id) else {
            return;
        };
        let (allowed, can_add, drawable) = {
            let Ok(obj) = obj_ref.read() else {
                return;
            };
            let allowed = obj.is_mass_selectable() || create_new_selection;
            let can_add = obj.get_ai_update_interface().is_some()
                || obj.is_any_kind_of(&[KindOf::Structure, KindOf::AlwaysSelectable]);
            let drawable = if affect_client {
                obj.get_drawable()
            } else {
                None
            };
            (allowed, can_add, drawable)
        };
        if !allowed {
            return;
        }
        crate::helpers::TheGameLogic::apply_select_object(
            object_id,
            create_new_selection,
            player_mask,
            can_add,
            affect_client,
            drawable,
        );
    }

    /// PARITY_NOTE: GameLogic::deselectObject(Object*, PlayerMaskType, Bool)
    /// C++ `GameLogic.cpp:2646-2690`.
    pub fn deselect_object(
        &mut self,
        object_id: ObjectID,
        player_mask: PlayerMaskType,
        affect_client: bool,
    ) {
        let Some(obj_ref) = self.find_object_by_id(object_id) else {
            return;
        };
        let drawable = {
            let Ok(obj) = obj_ref.read() else {
                return;
            };
            if affect_client {
                obj.get_drawable()
            } else {
                None
            }
        };
        crate::helpers::TheGameLogic::apply_deselect_object(
            object_id,
            player_mask,
            affect_client,
            drawable,
        );
    }

    // =========================================================================
    // C++ Parity: startNewGame / loadMapINI
    // =========================================================================

    /// PARITY_NOTE: GameLogic::startNewGame(Bool loadingSaveGame) C++ line 1081.
    /// Entry point for starting a new game or loading a save game.
    /// When not loading a save, sets the start-new-game flag so the actual
    /// load happens in the next update() call (after any intro movie).
    pub fn start_new_game(&mut self, loading_save_game: bool) {
        self.set_loading_map(true);
        if !loading_save_game {
            let map_path = game_engine::common::ini::get_global_data()
                .map(|data| data.read().map_name.clone())
                .unwrap_or_default();
            if !map_path.is_empty() {
                let mut state = game_engine::System::get_game_state();
                state.set_pristine_map_name(map_path);
            }
            if !crate::helpers::TheGameLogic::is_start_new_game_requested() {
                crate::helpers::TheGameLogic::request_start_new_game();
                return;
            }
        }
        clear_campaign_victorious_for_new_game();

        self.rank_level_limit = 1000;
        self.set_defaults(loading_save_game);
        self.show_behind_building_markers = true;
        self.draw_icon_ui = true;
        self.show_dynamic_lod = true;
        set_fp_mode();
        if let Some(client) = TheGameClient::get() {
            client.set_frame(0);
        }
        self.frame = 0;
        self.set_loading_map(false);
    }

    /// PARITY_NOTE: GameLogic::loadMapINI(AsciiString mapName) C++ line 2367.
    /// Loads map-specific INI overrides (map.ini, solo.ini, map.str).
    pub fn load_map_ini(&self, map_name: &str) {
        crate::system::game_initialization::GameInitializer::load_map_ini(map_name);
    }


    // =========================================================================
    // C++ Parity: bindObjectAndDrawable / sendObjectDestroyed
    // =========================================================================

    /// PARITY_NOTE: GameLogic::bindObjectAndDrawable(Object*, Drawable*) C++ line 4125.
    pub fn bind_object_and_drawable(&self, object_id: ObjectID, drawable_id: ObjectID) {
        bind_object_and_drawable(object_id, drawable_id);
    }

    /// PARITY_NOTE: GameLogic::sendObjectDestroyed(Object*) C++ line 4134.
    pub fn send_object_destroyed(&self, object_id: ObjectID) {
        if let Some(client) = TheGameClient::get() {
            client.clear_object_model_draws(object_id);
        }
        trace!("sendObjectDestroyed: obj={}", object_id);
    }
}

/// C++ GameLogic.cpp:1254-1256 TheCampaignManager->SetVictorious(FALSE).
fn clear_campaign_victorious_for_new_game() {
    if let Ok(mut guard) = get_script_engine().write() {
        if let Some(engine) = guard.as_mut() {
            engine.set_campaign_victorious(false);
        }
    }
}

/// C++ GameLogic.cpp:2073-2119 Challenge copies ThePlayer alliances onto the local general.
fn apply_challenge_the_player_relationships() {
    if !crate::scripting::core::is_generals_challenge_campaign() {
        return;
    }
    let Ok(list) = player_list().read() else {
        return;
    };
    let Some(local_arc) = list.get_local_player().cloned() else {
        return;
    };
    if let Some(placeholder_arc) = list.find_player_by_name(crate::scripting::core::THE_PLAYER) {
        let enemies: Vec<Arc<RwLock<Player>>> = {
            let Ok(placeholder) = placeholder_arc.read() else {
                return;
            };
            list.iter()
                .filter_map(|player_arc| {
                    if Arc::ptr_eq(player_arc, &placeholder_arc) {
                        return None;
                    }
                    let other = player_arc.read().ok()?;
                    (placeholder.get_relationship(&other) == crate::common::Relationship::Enemies)
                        .then(|| Arc::clone(player_arc))
                })
                .collect()
        };
        for enemy_arc in enemies {
            if Arc::ptr_eq(&enemy_arc, &local_arc) {
                continue;
            }
            if let (Ok(mut local), Ok(mut enemy)) = (local_arc.write(), enemy_arc.write()) {
                local.set_player_relationship(&enemy, crate::common::Relationship::Enemies);
                enemy.set_player_relationship(&local, crate::common::Relationship::Enemies);
            }
        }
        return;
    }

    let civilian = list.find_player_by_name("PlyrCivilian");
    let neutral = list.get_neutral_player();
    let others: Vec<Arc<RwLock<Player>>> = list.iter().cloned().collect();
    for other_arc in others {
        let rel = if Arc::ptr_eq(&other_arc, &local_arc) {
            crate::common::Relationship::Allies
        } else if civilian.as_ref().is_some_and(|c| Arc::ptr_eq(&other_arc, c))
            || neutral.as_ref().is_some_and(|n| Arc::ptr_eq(&other_arc, n))
        {
            crate::common::Relationship::Neutral
        } else {
            crate::common::Relationship::Enemies
        };
        if Arc::ptr_eq(&other_arc, &local_arc) {
            if let Ok(mut local) = local_arc.write() {
                let index = local.get_player_index();
                local.set_player_relationship_by_index(index, rel);
            }
            continue;
        }
        if let (Ok(mut local), Ok(other)) = (local_arc.write(), other_arc.read()) {
            local.set_player_relationship(&other, rel);
        }
    }
}

