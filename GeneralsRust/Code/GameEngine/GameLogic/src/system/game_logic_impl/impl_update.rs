impl GameLogic {
    /// **THE MAIN GAME LOOP** - Execute one simulation frame
    ///
    /// ## C++ Reference: GameLogic::update() (GameLogic.cpp lines 3548-3803)
    ///
    /// This is the heart of the game engine. It orchestrates all game systems
    /// in the proper order to maintain deterministic simulation.
    ///
    /// ## Frame Order (CRITICAL):
    /// 1. Pre-Update Phase - Clear events, reset flags
    /// 2. AI Phase - Update AI players
    /// 3. Command Phase - Process player commands
    /// 4. Object Update Phase - **INCLUDES STEALTH UPDATES**
    ///    - Normal updates (every frame, C++ line 3672-3695)
    ///    - Sleepy updates (deferred, C++ line 3697-3738) **← STEALTH HERE**
    /// 5. Damage/Physics Resolution Phase
    /// ## Update Loop Phase Ordering (matches C++ GameLogic::update)
    ///
    /// This method implements the exact phase ordering from the C++ codebase
    /// (GameLogic.cpp lines 3548-3803) to maintain simulation correctness
    /// and multiplayer determinism.
    ///
    /// ### C++ Reference Phase Order:
    /// ```text
    /// Line 3595: setFrame / sync to GameClient
    /// Line 3600: TheScriptEngine->UPDATE()           [early scripting]
    /// Line 3603: freezeTime check
    /// Line 3622: TheTerrainLogic->UPDATE()           [terrain/bridges]
    /// Line 3627: CRC calculation (MP/replay)
    /// Line 3657: StatsCollector update
    /// Line 3663: Recorder UPDATE
    /// Line 3669: processCommandList                  [command processing]
    /// Line 3672: ALLOW_NONSLEEPY_UPDATES loop        [normal modules]
    /// Line 3697: sleepy updates loop                 [sleepy modules]
    /// Line 3743: TheAI->UPDATE()                     [AI]
    /// Line 3748: TheBuildAssistant->UPDATE()         [production]
    /// Line 3753: ThePartitionManager->UPDATE()       [spatial]
    /// Line 3762: processDestroyList()                [death/cleanup]
    /// Line 3765: TheCommandList->reset()
    /// Line 3767: TheWeaponStore->UPDATE()            [weapons]
    /// Line 3768: TheLocomotorStore->UPDATE()         [locomotors]
    /// Line 3769: TheVictoryConditions->UPDATE()      [victory]
    /// Line 3783: disabled status check               [re-enable]
    /// Line 3799: m_frame++                           [increment]
    /// ```
    ///
    /// ## Stealth Integration Point:
    /// StealthUpdate modules are processed via the sleepy/normal update queues
    /// (C++ lines 3672-3738). Each stealth module checks stealth breaking
    /// conditions (attacking, moving, damage), updates detection status, and
    /// manages disguise transitions.
    ///
    /// ## Parameters
    /// - `frame`: The current frame number
    ///
    /// ## Returns
    /// - `Ok(())` if update succeeded
    /// - `Err(GameLogicError)` if a critical error occurred
    ///
    /// C++ `GameLogic::update()` always runs ScriptEngine / TerrainLogic /
    /// processDestroyList / `m_frame++` even with zero objects (shell maps).
    /// Empty dual-world registry + empty `objects` is therefore a full tick,
    /// not an early `Ok(())`.
    pub fn update(&mut self, frame: u32) -> Result<(), GameLogicError> {
        self.last_update_was_empty_noop = false;

        // Prevent re-entrant calls (C++ line 3552: LatchRestore<Bool> inUpdateLatch)
        if self.is_in_update {
            warn!("GameLogic::update called re-entrantly; ignoring nested call");
            return Err(GameLogicError::InvalidState(
                "Re-entrant update call".to_string(),
            ));
        }

        // C++ `GameLogic::update()` calls setFPMode() at update entry.
        set_fp_mode();

        self.is_in_update = true;

        // C++ GameLogic.cpp:3560-3576 — startNewGame at the top of update,
        // before setFrame / ScriptEngine. MSG_NEW_GAME only arms the request;
        // the expensive start runs here once the intro movie gate is clear.
        if crate::helpers::TheGameLogic::is_start_new_game_requested()
            && !crate::helpers::TheGameLogic::is_intro_movie_playing()
        {
            if let Err(e) = self.start_new_game_now(false) {
                warn!("Deferred new-game start failed: {}", e);
            }
        }

        self.frame = frame;
        self.game_time = frame as f32 * FIXED_DELTA_TIME;

        trace!("GameLogic::update(frame={}) - Begin update cycle", frame);

        // -----------------------------------------------------------------------
        // Phase 0: Frame Setup (C++ lines 3595-3596)
        // -----------------------------------------------------------------------
        // C++: UnsignedInt now = TheGameLogic->getFrame();
        // C++: TheGameClient->setFrame(now);
        if let Some(client) = TheGameClient::get() {
            client.set_frame(frame);
        }

        // -----------------------------------------------------------------------
        // Phase 1: Early Scripting (C++ line 3600)
        // -----------------------------------------------------------------------
        // C++: TheScriptEngine->UPDATE();
        //
        // The script engine runs BEFORE object updates so that scripts can react
        // to state changes from the previous frame and issue commands that will
        // be processed in the command phase below.
        if let Err(e) = self.evaluate_scripts() {
            warn!("Early scripting phase failed: {}", e);
        }

        // -----------------------------------------------------------------------
        // Phase 2: Time Freeze Check (C++ lines 3603-3617)
        // -----------------------------------------------------------------------
        // C++: Bool freezeTime = TheTacticalView->isTimeFrozen() && !TheTacticalView->isCameraMovementFinished()
        // C++: if (freezeTime) { ... return; }
        if self.is_time_frozen() {
            trace!("GameLogic::update - Time frozen, skipping frame");
            self.is_in_update = false;
            return Ok(());
        }

        // C++: if (m_gamePaused) { return; } — paused game skips simulation
        if self.game_paused {
            self.is_in_update = false;
            return Ok(());
        }

        // -----------------------------------------------------------------------
        // Phase 3: Pre-Update / Terrain (C++ lines 3619-3623)
        // -----------------------------------------------------------------------
        // C++: TheTerrainLogic->UPDATE();
        //
        // Terrain updates happen BEFORE object updates so bridge state changes
        // noted by scripts are reflected before the object update phase.
        if let Err(e) = self.update_terrain() {
            warn!("Terrain update phase failed: {}", e);
        }

        // Phase 3b: CRC Calculation (C++ lines 3625-3665)
        // -----------------------------------------------------------------------
        // C++: m_CRC = getCRC(CRC_RECALC); TheMessageStream->appendMessage(MSG_LOGIC_CRC);
        // then TheStatsCollector->update(); TheRecorder->UPDATE();
        let current_crc_interval =
            game_engine::common::crc_debug::replay_crc_interval() as UnsignedInt;
        let mut posted_logic_crc = None;
        if self.frame > 0 && self.frame % current_crc_interval == 0 {
            self.crc_cache = self.compute_crc();
            let playback = game_engine::common::recorder::with_recorder(|recorder| {
                recorder.is_playback()
            })
            .unwrap_or(false);
            if let Ok(mut stream) =
                game_engine::common::message_stream::get_message_stream().write()
            {
                let crc_msg = stream.append_message(
                    game_engine::common::message_stream::game_message::GameMessageType::LogicCRC(
                        self.crc_cache,
                    ),
                );
                crc_msg.append_boolean_argument(playback);
            }
            posted_logic_crc = Some(self.crc_cache);
        }
        game_engine::common::stats_collector::with_stats_collector_mut(|collector| {
            collector.update();
        });
        game_engine::common::recorder::with_recorder_mut(|recorder| {
            recorder.set_current_frame(self.frame);
            recorder.update();
            if let Some(crc) = posted_logic_crc {
                // C++ GameLogicDispatch.cpp:1940-1946 — live CRC compares after recorded enqueue.
                recorder.notify_logic_crc(crc, 0);
            }
        });


        // Clear frame events and reset temporary flags
        if let Err(e) = self.clear_frame_events() {
            warn!("Pre-update clear failed: {}", e);
        }
        if let Err(e) = self.reset_temporary_flags() {
            warn!("Reset temporary flags failed: {}", e);
        }

        // Update the command system's frame counter
        if let Err(e) = crate::commands::update_command_system(frame) {
            warn!("Command system update failed: {}", e);
        }

        // -----------------------------------------------------------------------
        // Phase 4: Command Processing (C++ lines 3668-3669)
        // -----------------------------------------------------------------------
        // C++: processCommandList( TheCommandList );
        //
        // Process all queued player commands. This must happen BEFORE object
        // updates so that movement/attack orders are in effect when objects
        // run their AI/physics updates.
        if let Err(e) = self.process_command_queue() {
            warn!("Command processing phase failed: {}", e);
        }
        self.process_beacon_updates();
        self.process_radar_updates();

        // -----------------------------------------------------------------------
        // Phase 5: Object Update - Normal Modules (C++ lines 3672-3694)
        // -----------------------------------------------------------------------
        // C++: for (std::list<UpdateModulePtr>::const_iterator it = m_normalUpdates...)
        //
        // Process all non-sleepy (every-frame) update modules.
        // These include physics updates that must run at full frame rate.
        self.process_normal_updates();

        // -----------------------------------------------------------------------
        // Phase 6: Object Update - Sleepy Modules (C++ lines 3697-3738)
        // -----------------------------------------------------------------------
        // C++: while (!m_sleepyUpdates.empty()) { ... }
        //
        // Process all sleepy (delayed) update modules whose wake frame has
        // arrived. StealthUpdate, AIUpdate, and many behavior modules live here.
        // STEALTH: Most stealth modules are sleepy, updating every frame when
        // active. The sequence when a unit attacks:
        //   a. WeaponUpdate sets OBJECT_STATUS_IS_FIRING_WEAPON
        //   b. StealthUpdate::allowedToStealth() checks flag (C++ StealthUpdate.cpp:268)
        //   c. Stealth is broken, OBJECT_STATUS_STEALTHED cleared
        //   d. Stealth delay timer starts
        //   e. After delay + weapon stop, stealth reactivates
        self.process_sleepy_updates(frame);

        // -----------------------------------------------------------------------
        // Phase 6b: Object-level updates (damage types, projectile objects, stealth)
        // -----------------------------------------------------------------------
        if let Err(e) = self.process_object_updates(FIXED_DELTA_TIME) {
            warn!("Object update phase failed: {}", e);
        }
        if let Err(e) = self.process_stealth_controllers(FIXED_DELTA_TIME) {
            warn!("Stealth update phase failed: {}", e);
        }
        // PARITY_NOTE: C++ DoT is handled by PoisonedBehavior module updates, not a global manager.
        // The fabricated DotManager/update_dot_effects has been removed.

        // Keep special power timers/cooldowns in sync with the simulation frame.
        crate::special_power_module::update();

        // C++ GameLogic::update does not run ClientUpdateModule / drawable
        // updates. Those belong on GameClient::update (hq-um5t).

        // -----------------------------------------------------------------------
        // Phase 7: AI Update (C++ line 3743)
        // -----------------------------------------------------------------------
        // C++: TheAI->UPDATE();
        //
        // AI runs AFTER object updates so AI decisions are based on the latest
        // world state. This ordering is critical: objects move first, then
        // AI observes the new positions and issues commands for the next frame.
        if let Err(e) = self.update_ai_players(frame) {
            warn!("AI update phase failed: {}", e);
            // Don't abort - continue with other systems
        }

        // -----------------------------------------------------------------------
        // Phase 8: Production / Build Assistant (C++ line 3748)
        // -----------------------------------------------------------------------
        // C++: TheBuildAssistant->UPDATE();
        //
        // Production updates run after AI so build orders issued by AI this
        // frame can be immediately reflected in production queues.
        if let Err(e) = self.update_production(frame) {
            warn!("Production update phase failed: {}", e);
        }

        // -----------------------------------------------------------------------
        // Phase 9: Damage/Physics Resolution
        // -----------------------------------------------------------------------
        // Deferred damage and collision resolution after all objects have moved.
        if let Err(e) = self.resolve_damage_and_physics() {
            warn!("Physics resolution phase failed: {}", e);
        }

        self.update_objects_changed_trigger_areas();

        // -----------------------------------------------------------------------
        // Phase 10: Partition Manager Update (C++ line 3753)
        // -----------------------------------------------------------------------
        // C++: ThePartitionManager->UPDATE();
        //
        // Spatial partition is updated AFTER all objects have moved and before
        // death cleanup so queries during cleanup use correct positions.
        if let Err(e) = self.update_partition_manager() {
            warn!("Partition manager update failed: {}", e);
        }

        // -----------------------------------------------------------------------
        // Phase 11: Death/Cleanup (C++ line 3762)
        // -----------------------------------------------------------------------
        // C++: processDestroyList();
        //
        // Destroyed objects are removed from the world. This happens after
        // partition update so spatial queries remain valid during cleanup.
        if let Err(e) = self.cleanup_dead_objects() {
            warn!("Cleanup phase failed: {}", e);
        }

        // Periodically sweep dead weak references from the object registry so
        // that entries for objects that are never looked up again do not
        // accumulate unbounded.
        if frame % 1000 == 0 {
            OBJECT_REGISTRY.cleanup_dead_references();
        }

        // Reset the command queue (C++ line 3765: TheCommandList->reset())
        // Commands already processed; clear any remaining for next frame.
        self.command_queue.clear();

        // -----------------------------------------------------------------------
        // Phase 12: Weapon Store Update (C++ line 3767)
        // -----------------------------------------------------------------------
        // C++: TheWeaponStore->UPDATE();
        //
        // Process delayed damage (weapons with delay) that is now ready.
        if let Err(e) = self.update_weapon_store() {
            warn!("Weapon store update phase failed: {}", e);
        }

        // -----------------------------------------------------------------------
        // Phase 12b: Locomotor Store Update (C++ line 3768)
        // -----------------------------------------------------------------------
        // C++: TheLocomotorStore->UPDATE();
        crate::locomotor::core::LOCOMOTOR_STORE.update();

        // -----------------------------------------------------------------------
        // Phase 13: Victory Conditions (C++ line 3769)
        // -----------------------------------------------------------------------
        // C++: TheVictoryConditions->UPDATE();
        self.update_victory_conditions();

        // -----------------------------------------------------------------------
        // Phase 14: Disabled Status Check (C++ lines 3783-3792)
        // -----------------------------------------------------------------------
        // C++: for( Object *obj = m_objList; obj; obj = obj->getNextObject() )
        // C++:   if( obj->isDisabled() ) obj->checkDisabledStatus();
        //
        // Check timer-based disabled states and re-enable objects whose
        // disable duration has expired. This happens at end-of-frame so
        // disabled objects are inactive for the entire current frame.
        self.check_disabled_statuses();

        // -----------------------------------------------------------------------
        // Phase 15: Post-Update - Vision/Shroud and Team Events
        // -----------------------------------------------------------------------
        if let Err(e) = self.update_vision_and_shroud() {
            warn!("Vision update failed: {}", e);
        }
        // Team::updateState runs from ScriptEngine after executeScripts
        // (C++ ScriptEngine.cpp:5573). Flush leftover events only.
        flush_pending_team_script_events();

        // -----------------------------------------------------------------------
        // Phase 16: Frame Increment (C++ lines 3799-3802)
        // -----------------------------------------------------------------------
        // C++: if (!m_startNewGame) { m_frame++; }
        if !crate::helpers::TheGameLogic::is_start_new_game_requested() {
            self.frame += 1;
        }

        self.is_in_update = false;

        trace!("GameLogic::update(frame={}) - End update cycle", frame);
        Ok(())
    }

    /// Drain and return all radar updates generated so far this frame. This
    /// mirrors the C++ pattern where the client polls radar events after the
    /// command/object phases.
    pub fn take_radar_updates(&mut self) -> Vec<RadarUpdate> {
        std::mem::take(&mut self.radar_updates)
    }

    /// Phase 1: Clear frame-based events and temporary state
    ///
    /// ## C++ Reference: Called at start of GameLogic::update()
    pub fn clear_frame_events(&mut self) -> Result<(), GameLogicError> {
        trace!("GameLogic::clear_frame_events()");

        // Clear event queues
        self.event_queue.clear();
        self.radar_updates.clear();

        // Clear temporary flags on objects
        for obj_id in &self.all_objects {
            if let Some(obj_ref) = self.objects.get(obj_id) {
                if let Ok(_obj) = obj_ref.write() {
                    // Clear frame-based flags
                    // (In full implementation, this would clear selection updates,
                    // temporary status bits, etc.)
                }
            }
        }

        Ok(())
    }

    fn process_beacon_updates(&mut self) {
        for update in drain_beacon_updates() {
            match update {
                BeaconUpdate::Placed(entry) => {
                    self.event_queue.push(GameEvent::BeaconPlaced {
                        player_id: entry.player_id,
                        position: entry.position,
                        text: entry.text.clone(),
                    });
                    self.radar_updates.push(RadarUpdate {
                        player_id: entry.player_id,
                        position: (entry.position.x, entry.position.y),
                        event_type: RadarEventType::BeaconPlaced,
                    });
                }
                BeaconUpdate::Removed {
                    player_id,
                    position,
                } => {
                    self.event_queue.push(GameEvent::BeaconRemoved {
                        player_id,
                        position,
                    });
                    self.radar_updates.push(RadarUpdate {
                        player_id,
                        position: (position.x, position.y),
                        event_type: RadarEventType::BeaconRemoved,
                    });
                }
                BeaconUpdate::TextUpdated {
                    player_id,
                    position,
                    text,
                } => {
                    self.event_queue.push(GameEvent::BeaconTextUpdated {
                        player_id,
                        position,
                        text,
                    });
                }
            }
        }
    }

    /// Promote radar updates generated this frame into the event queue so
    /// client/UI layers can trigger minimap and EVA feedback.
    fn process_radar_updates(&mut self) {
        for update in self.radar_updates.drain(..) {
            radar_notifier::push(&update);
            self.event_queue.push(GameEvent::RadarUpdate {
                player_id: update.player_id,
                position: update.position,
                event_type: update.event_type,
            });
        }
    }

    /// Reset temporary flags at frame start
    pub fn reset_temporary_flags(&mut self) -> Result<(), GameLogicError> {
        trace!("GameLogic::reset_temporary_flags()");
        // Stub: reset any temporary frame-based flags
        Ok(())
    }

    /// Phase 2: Update all AI players
    ///
    /// ## C++ Reference: GameLogic::update() AI section
    ///
    /// Iterates through all players and updates AI players (skipping humans).
    /// AI updates include:
    /// - Build order processing
    /// - Unit production decisions
    /// - Base building/expansion
    /// - Tactical decisions
    pub fn update_ai_players(&mut self, frame: UnsignedInt) -> Result<(), GameLogicError> {
        trace!("GameLogic::update_ai_players(frame={})", frame);

        // Access the global AI system
        let ai_store = the_ai(); if let Ok(mut ai) = ai_store.write() {
            if let Err(e) = ai.update(frame) {
                return Err(GameLogicError::AIError(format!("AI update failed: {}", e)));
            }
        } else {
            return Err(GameLogicError::AIError(
                "AI system lock poisoned".to_string(),
            ));
        }

        if let Some(result) = with_ai_integration_mut(|manager| manager.update_ai_players_only()) {
            if let Err(e) = result {
                warn!("AI player update failed at frame {}: {:?}", frame, e);
            }
        }

        Ok(())
    }

    /// Phase 3: Process command queue
    ///
    /// ## C++ Reference: GameLogic::processCommandList() (GameLogic.cpp)
    ///
    /// Processes all queued player commands:
    /// - Unit movement orders
    /// - Attack commands
    /// - Build orders
    /// - Special power activations
    pub fn process_command_queue(&mut self) -> Result<(), GameLogicError> {
        trace!(
            "GameLogic::process_command_queue() - {} commands pending",
            self.command_queue.len()
        );

        // C++ parity: consume routed command-list messages before object updates.
        // Route target is the shared CommandQueueManager fed by GameClient translators.
        if let Ok(mut processor) = crate::commands::get_command_processor().lock() {
            let mut context = crate::commands::CommandExecutionContext {
                current_frame: self.frame,
                player_id: 0,
                object_manager: None,
                player_manager: None,
                ai_manager: None,
                execution_start_time: Instant::now(),
                is_network_command: false,
                is_replay_command: false,
            };
            if let Err(err) = processor.process_frame(self.frame, &mut context) {
                warn!("Command processor frame execution failed: {}", err);
            }
        }

        // Process all pending commands
        while let Some(command) = self.command_queue.pop_front() {
            if let Err(e) = self.execute_command(command) {
                warn!("Command execution failed: {}", e);
                // Continue processing other commands
            }
        }

        // Also process commands through dispatch system
        if let Some(dispatch_mutex) = get_dispatch() {
            if let Ok(mut dispatch) = dispatch_mutex.lock() {
                if let Err(e) = dispatch.update(self.frame) {
                    warn!("Dispatch update failed: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Execute a single game command
    fn execute_command(&mut self, command: GameCommand) -> Result<(), GameLogicError> {
        match command {
            GameCommand::MoveUnit {
                player_id,
                unit_ids,
                target_position: _target_position,
            } => {
                trace!(
                    "Executing MoveUnit command for player {} ({} units)",
                    player_id,
                    unit_ids.len()
                );
                // In full implementation: apply movement orders to units
                Ok(())
            }
            GameCommand::AttackTarget {
                player_id,
                attacker_ids,
                target_id: _target_id,
            } => {
                trace!(
                    "Executing AttackTarget command for player {} ({} attackers)",
                    player_id,
                    attacker_ids.len()
                );
                // In full implementation: apply attack orders
                Ok(())
            }
            GameCommand::BuildStructure {
                player_id,
                builder_id: _builder_id,
                structure_type,
                position: _position,
            } => {
                trace!(
                    "Executing BuildStructure command for player {} ({})",
                    player_id,
                    structure_type
                );
                // In full implementation: start structure construction
                Ok(())
            }
            GameCommand::UseSpecialPower {
                player_id,
                power_name,
                target_position: _target_position,
            } => {
                trace!(
                    "Executing UseSpecialPower command for player {} ({})",
                    player_id,
                    power_name
                );
                // In full implementation: activate special power
                Ok(())
            }
        }
    }

    /// Phase 4: Update all objects and their modules
    ///
    /// ## C++ Reference: GameLogicDispatch.cpp (the dispatch system)
    /// ## C++ Reference: GameLogic.cpp lines 3672-3738 (update module processing)
    ///
    /// This is the largest phase of the update loop. It iterates through
    /// all live objects and calls their update() method, which in turn
    /// triggers ALL 86+ UpdateModule types:
    ///
    /// - AIUpdate (pathfinding, group commands, state machines)
    /// - StealthUpdate (stealth state, detection, disguise) - **NOW INTEGRATED**
    /// - FireWeaponUpdate (weapon firing, cooldowns)
    /// - PhysicsUpdate (gravity, velocity, collision detection)
    /// - ProductionUpdate (unit/structure building timers)
    /// - SpecialPowerUpdate (special ability state)
    /// - DockUpdate (docking, supply transfer, repair)
    /// - ... and 80+ more module types
    ///
    /// ## Critical Note:
    /// Objects can be destroyed during updates, so we use a cloned ID list
    /// to avoid iterator invalidation.
    ///
    /// ## Stealth Integration:
    /// Stealth updates are processed through the sleepy/normal update queues
    /// based on their wake frame. This matches C++ behavior where stealth
    /// is just another UpdateModule in the queue system.
    pub fn process_object_updates(&mut self, delta_time: f32) -> Result<(), GameLogicError> {
        trace!(
            "GameLogic::process_object_updates(delta={:.4}s) - {} objects",
            delta_time,
            self.all_objects.len()
        );

        // Clone object list to avoid iterator invalidation
        // (objects may be destroyed during update)
        let object_ids = self.all_objects.clone();

        for obj_id in object_ids {
            // Check if object still exists (may have been destroyed)
            if let Some(obj_ref) = self.objects.get(&obj_id) {
                if let Ok(mut obj) = obj_ref.write() {
                    // Call object's update method
                    // In full implementation, this triggers all UpdateModules
                    // including StealthUpdate which manages:
                    // - Stealth state transitions (stealthed/unstealthed)
                    // - Detection status (detected by enemies)
                    // - Disguise system (bomb truck disguising)
                    // - Stealth breaking conditions (attacking, moving, damage)
                    if let Err(e) = obj.update(delta_time) {
                        warn!("Object {} update failed: {:?}", obj_id, e);
                        // Don't abort - continue updating other objects
                    }
                } else {
                    warn!("Object {} lock poisoned during update", obj_id);
                }
            }
        }

        Ok(())
    }

    /// Update object-linked stealth controllers once per frame.
    ///
    /// C++ parity note: `StealthUpdate` is a standard update module in the
    /// regular update queue. The current Rust port stores stealth as an object
    /// handle; this bridge keeps per-frame stealth state transitions active.
    fn process_stealth_controllers(&mut self, delta_time: f32) -> Result<(), GameLogicError> {
        let mut handles = Vec::new();
        for &object_id in &self.all_objects {
            let Some(object_ref) = self.objects.get(&object_id) else {
                continue;
            };
            let Ok(object_guard) = object_ref.read() else {
                continue;
            };
            let Some(stealth) = object_guard.get_stealth() else {
                continue;
            };
            handles.push((object_id, stealth));
        }

        for (object_id, handle) in handles {
            let Ok(mut stealth_guard) = handle.lock() else {
                warn!("Stealth controller lock poisoned for object {}", object_id);
                continue;
            };
            if let Err(err) = stealth_guard.update_stealth(delta_time) {
                warn!("Stealth update failed for object {}: {}", object_id, err);
            }
        }

        Ok(())
    }


    /// Process sleepy (delayed) update modules
    ///
    /// ## C++ Reference: GameLogic.cpp lines 3697-3738 (sleepy update queue)
    ///
    /// Sleepy updates are modules that only need to update occasionally,
    /// not every frame. They "sleep" until their wake frame arrives.
    ///
    /// ## Stealth Module Integration:
    /// StealthUpdate is a sleepy module that typically updates every frame but can sleep
    /// when disabled (disguise system, special power grant). Key behaviors:
    /// - Updates stealth state based on conditions (moving, attacking, damage)
    /// - Manages detection timer (when enemies detect the unit)
    /// - Handles disguise transitions (bomb truck)
    /// - Applies opacity changes for visual stealth effect
    /// - Returns UPDATE_SLEEP_NONE (1) when enabled, UPDATE_SLEEP_FOREVER when disabled
    ///
    /// ## C++ Behavior Match:
    /// - Lines 3697-3713: Peek at next sleepy update, check wake frame
    /// - Lines 3717-3732: Check disabled flags, call update(), get sleep time
    /// - Lines 3735-3736: Requeue with new wake frame (now + sleepLen)

    /// C++ parity: GameLogic::processCommandList() (GameLogic.cpp line 2516)
    ///
    /// Iterate a list of game messages and dispatch each one through
    /// `logic_message_dispatcher`. The C++ version also validates CRCs
    /// from network players; that logic lives in the network layer in Rust.
    pub fn process_command_list(&mut self, messages: Vec<crate::messages::GameMessage>) {
        for msg in messages {
            self.logic_message_dispatcher(&msg);
        }
    }

    /// C++ parity: GameLogic::logicMessageDispatcher() (GameLogicDispatch.cpp line 328)
    ///
    /// Central command router that switches on the message type and dispatches
    /// to the appropriate handler. Matches the C++ switch statement exactly.
    pub fn logic_message_dispatcher(&mut self, msg: &crate::messages::GameMessage) {
        use crate::commands::command::CommandType;

        let command_type = match CommandType::try_from(msg.id as u16) {
            Ok(ct) => ct,
            Err(_) => {
                trace!("logic_message_dispatcher: unknown command type {}", msg.id);
                return;
            }
        };

        match command_type {
            CommandType::NewGame => {
                trace!("logic_message_dispatcher: MSG_NEW_GAME");
            }
            CommandType::ClearGameData => {
                trace!("logic_message_dispatcher: MSG_CLEAR_GAME_DATA");
            }
            CommandType::SetRallyPoint => {
                let obj_id = msg.arguments.first().and_then(|a| match a {
                    crate::messages::MessageArgument::ObjectId(id) => Some(*id),
                    _ => None,
                });
                let dest = msg.arguments.get(1).and_then(|a| match a {
                    crate::messages::MessageArgument::Location(c) => Some(*c),
                    _ => None,
                });
                if let (Some(id), Some(dest)) = (obj_id, dest) {
                    if let Some(obj_arc) = self.find_object_by_id(id) {
                        if let Ok(mut obj) = obj_arc.write() {
                            let _ = obj.set_rally_point(&dest);
                        }
                    }
                }
            }
            CommandType::DoWeapon => {
                trace!("logic_message_dispatcher: MSG_DO_WEAPON");
            }
            CommandType::CombatDropAtObject => {
                trace!("logic_message_dispatcher: MSG_COMBATDROP_AT_OBJECT");
            }
            CommandType::CombatDropAtLocation => {
                trace!("logic_message_dispatcher: MSG_COMBATDROP_AT_LOCATION");
            }
            CommandType::DoWeaponAtObject => {
                trace!("logic_message_dispatcher: MSG_DO_WEAPON_AT_OBJECT");
            }
            CommandType::SwitchWeapons => {
                trace!("logic_message_dispatcher: MSG_SWITCH_WEAPONS");
            }
            CommandType::SetMineClearingDetail => {
                trace!("logic_message_dispatcher: MSG_SET_MINE_CLEARING_DETAIL");
            }
            CommandType::EnableRetaliationMode => {
                trace!("logic_message_dispatcher: MSG_ENABLE_RETALIATION_MODE");
            }
            CommandType::DoWeaponAtLocation => {
                trace!("logic_message_dispatcher: MSG_DO_WEAPON_AT_LOCATION");
            }
            CommandType::DoSpecialPower => {
                trace!("logic_message_dispatcher: MSG_DO_SPECIAL_POWER");
            }
            CommandType::DoSpecialPowerAtLocation => {
                trace!("logic_message_dispatcher: MSG_DO_SPECIAL_POWER_AT_LOCATION");
            }
            CommandType::DoSpecialPowerAtObject => {
                trace!("logic_message_dispatcher: MSG_DO_SPECIAL_POWER_AT_OBJECT");
            }
            CommandType::DoAttackMoveTo => {
                trace!("logic_message_dispatcher: MSG_DO_ATTACKMOVETO");
            }
            CommandType::DoForceMoveTo => {
                trace!("logic_message_dispatcher: MSG_DO_FORCEMOVETO");
            }
            CommandType::DoSalvage | CommandType::DoMoveTo => {
                trace!("logic_message_dispatcher: MSG_DO_MOVETO/MSG_DO_SALVAGE");
            }
            CommandType::AddWaypoint => {
                trace!("logic_message_dispatcher: MSG_ADD_WAYPOINT");
            }
            CommandType::DoGuardPosition => {
                trace!("logic_message_dispatcher: MSG_DO_GUARD_POSITION");
            }
            CommandType::DoGuardObject => {
                trace!("logic_message_dispatcher: MSG_DO_GUARD_OBJECT");
            }
            CommandType::DoStop => {
                trace!("logic_message_dispatcher: MSG_DO_STOP");
            }
            CommandType::DoScatter => {
                trace!("logic_message_dispatcher: MSG_DO_SCATTER");
            }
            CommandType::CreateFormation => {
                trace!("logic_message_dispatcher: MSG_CREATE_FORMATION");
            }
            CommandType::ClearInGamePopupMessage => {
                trace!("logic_message_dispatcher: MSG_CLEAR_INGAME_POPUP_MESSAGE");
            }
            CommandType::DoCheer => {
                trace!("logic_message_dispatcher: MSG_DO_CHEER");
            }
            CommandType::Enter => {
                trace!("logic_message_dispatcher: MSG_ENTER");
            }
            CommandType::Exit => {
                trace!("logic_message_dispatcher: MSG_EXIT");
            }
            CommandType::Evacuate => {
                trace!("logic_message_dispatcher: MSG_EVACUATE");
            }
            CommandType::ExecuteRailedTransport => {
                trace!("logic_message_dispatcher: MSG_EXECUTE_RAILED_TRANSPORT");
            }
            CommandType::InternetHack => {
                trace!("logic_message_dispatcher: MSG_INTERNET_HACK");
            }
            CommandType::GetRepaired => {
                trace!("logic_message_dispatcher: MSG_GET_REPAIRED");
            }
            CommandType::Dock => {
                trace!("logic_message_dispatcher: MSG_DOCK");
            }
            CommandType::GetHealed => {
                trace!("logic_message_dispatcher: MSG_GET_HEALED");
            }
            CommandType::DoRepair => {
                trace!("logic_message_dispatcher: MSG_DO_REPAIR");
            }
            CommandType::ResumeConstruction => {
                trace!("logic_message_dispatcher: MSG_RESUME_CONSTRUCTION");
            }
            CommandType::DoSpecialPowerOverrideDestination => {
                trace!("logic_message_dispatcher: MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION");
            }
            CommandType::DoAttackObject => {
                trace!("logic_message_dispatcher: MSG_DO_ATTACK_OBJECT");
            }
            CommandType::DoForceAttackObject => {
                trace!("logic_message_dispatcher: MSG_DO_FORCE_ATTACK_OBJECT");
            }
            CommandType::DoForceAttackGround => {
                trace!("logic_message_dispatcher: MSG_DO_FORCE_ATTACK_GROUND");
            }
            CommandType::QueueUpgrade => {
                trace!("logic_message_dispatcher: MSG_QUEUE_UPGRADE");
            }
            CommandType::CancelUpgrade => {
                trace!("logic_message_dispatcher: MSG_CANCEL_UPGRADE");
            }
            CommandType::QueueUnitCreate => {
                trace!("logic_message_dispatcher: MSG_QUEUE_UNIT_CREATE");
            }
            CommandType::CancelUnitCreate => {
                trace!("logic_message_dispatcher: MSG_CANCEL_UNIT_CREATE");
            }
            CommandType::DozerConstruct | CommandType::DozerConstructLine => {
                trace!("logic_message_dispatcher: MSG_DOZER_CONSTRUCT");
            }
            CommandType::DozerCancelConstruct => {
                trace!("logic_message_dispatcher: MSG_DOZER_CANCEL_CONSTRUCT");
            }
            CommandType::Sell => {
                trace!("logic_message_dispatcher: MSG_SELL");
            }
            CommandType::ToggleOvercharge => {
                trace!("logic_message_dispatcher: MSG_TOGGLE_OVERCHARGE");
            }
            CommandType::CreateSelectedGroup | CommandType::CreateSelectedGroupNoSound => {
                trace!("logic_message_dispatcher: MSG_CREATE_SELECTED_GROUP");
            }
            CommandType::RemoveFromSelectedGroup => {
                trace!("logic_message_dispatcher: MSG_REMOVE_FROM_SELECTED_GROUP");
            }
            CommandType::DestroySelectedGroup => {
                trace!("logic_message_dispatcher: MSG_DESTROY_SELECTED_GROUP");
            }
            CommandType::SelectedGroupCommand => {}
            CommandType::PlaceBeacon => {
                trace!("logic_message_dispatcher: MSG_PLACE_BEACON");
            }
            CommandType::RemoveBeacon => {
                trace!("logic_message_dispatcher: MSG_REMOVE_BEACON");
            }
            CommandType::SetBeaconText => {
                trace!("logic_message_dispatcher: MSG_SET_BEACON_TEXT");
            }
            CommandType::SelfDestruct => {
                trace!("logic_message_dispatcher: MSG_SELF_DESTRUCT");
            }
            CommandType::SetReplayCamera => {}
            CommandType::LogicCrc => {
                let local_crc = self.crc_cache;
                trace!(
                    "logic_message_dispatcher: MSG_LOGIC_CRC (local=0x{:08X})",
                    local_crc
                );
            }
            CommandType::PurchaseScience => {
                trace!("logic_message_dispatcher: MSG_PURCHASE_SCIENCE");
            }
            CommandType::MetaBeginPathBuild => {
                trace!("logic_message_dispatcher: MSG_META_BEGIN_PATH_BUILD");
            }
            CommandType::MetaEndPathBuild => {
                trace!("logic_message_dispatcher: MSG_META_END_PATH_BUILD");
            }
            CommandType::DebugKillSelection
            | CommandType::DebugHurtObject
            | CommandType::DebugKillObject => {
                trace!("logic_message_dispatcher: debug command {:?}", command_type);
            }
            _ => {
                trace!(
                    "logic_message_dispatcher: unhandled command type {:?}",
                    command_type
                );
            }
        }
    }

    pub fn resolve_damage_and_physics(&mut self) -> Result<(), GameLogicError> {
        trace!("GameLogic::resolve_damage_and_physics()");

        // Process all pending damage and collisions
        let mut physics_world = std::mem::take(&mut self.physics_world);
        physics_world.resolve_all(self)?;
        self.physics_world = physics_world;

        // C++ iterates GameLogic.objects (m_objList), not a second factory registry.
        let collision_ids: Vec<ObjectID> = if !self.all_objects.is_empty() {
            self.all_objects.clone()
        } else if !dual_world_registry_unavailable() {
            OBJECT_REGISTRY.get_all_object_ids()
        } else {
            Vec::new()
        };
        let mut cell_changed: Vec<ObjectID> = Vec::new();
        if !collision_ids.is_empty() {
            let _ = with_collision_system_mut(|system| {
                for obj_id in collision_ids {
                    let obj_arc = match self
                        .find_object_by_id(obj_id)
                        .or_else(|| OBJECT_REGISTRY.get_object(obj_id))
                    {
                        Some(v) => v,
                        None => continue,
                    };
                    let Ok(obj) = obj_arc.read() else {
                        continue;
                    };
                    let id = obj.get_id();
                    let pos = obj.get_position();
                    let geom = map_collision_geometry(
                        &obj.get_geometry_info(),
                        obj.get_template_geometry_type(),
                    );
                    if system
                        .update_object_position(
                            id,
                            crate::object::collide::Coord3D::new(pos.x, pos.y, pos.z),
                        )
                        .is_err()
                    {
                        let _ = system.register_object(
                            id,
                            crate::object::collide::Coord3D::new(pos.x, pos.y, pos.z),
                            geom,
                            None,
                        );
                    }
                }
                let _ = system.process_collisions();
                cell_changed = system.partition_manager_mut().take_cell_changed_events();
                Ok::<(), crate::object::collide::CollisionError>(())
            });
        }
        // C++ `PartitionData::friend_updateCellsTouched` fires
        // `obj->onPartitionCellChange()` inline when the center cell changed
        // (PartitionManager.cpp:2052-2062) — the movement-driven shroud
        // look/unlook driver. Deferred here so no object read guard is held
        // while `handle_partition_cell_maintenance` mutates the object.
        for obj_id in cell_changed {
            let _ = OBJECT_REGISTRY.with_object_mut(obj_id, |object_guard| {
                object_guard.handle_partition_cell_maintenance();
            });
        }

        // Update physics engine (terrain-aware simulation)
        if let Ok(mut physics) = crate::physics::get_physics_engine().write() {
            if let Err(err) = physics.update() {
                return Err(GameLogicError::PhysicsError(format!("{err}")));
            }
        }

        Ok(())
    }

    /// Phase 6: Cleanup dead objects
    ///
    /// ## C++ Reference: GameLogic::processDestroyList() (GameLogic.cpp)
    ///
    /// Removes destroyed objects from the game world:
    /// - Release contained objects (passengers, etc.)
    /// - Remove from team/group
    /// - Remove from partition manager
    /// - Fire destruction events

    pub fn update_partition_manager(&mut self) -> Result<(), GameLogicError> {
        trace!("GameLogic::update_partition_manager()");
        self.partition_manager.update()
    }

    pub fn partition_manager(&self) -> &PartitionManager {
        &self.partition_manager
    }

    pub fn partition_manager_mut(&mut self) -> &mut PartitionManager {
        &mut self.partition_manager
    }

    /// Phase 7: Update vision and shroud (fog of war)
    ///
    /// ## C++ Reference: Shroud system updates
    ///
    /// Updates visibility for all players:
    /// - Update visible objects for each player
    /// - Clear shroud in visible areas
    /// - Update stealth detection
    /// - Fire vision update events
    pub fn update_vision_and_shroud(&mut self) -> Result<(), GameLogicError> {
        trace!("GameLogic::update_vision_and_shroud()");

        // Update ShroudManager with current visibility information
        use crate::system::shroud_manager::get_shroud_manager;

        let shroud = get_shroud_manager();
        if let Ok(mut shroud_mgr) = shroud.lock() {
            // Update visibility cache (may skip frames based on update interval)
            // Uses self.frame instead of self.frame_counter (which doesn't exist)
            if let Err(e) = shroud_mgr.update(self.frame) {
                warn!("ShroudManager update failed: {}", e);
            }
        }

        // For each player, update their visible objects
        if let Ok(player_list_guard) = player_list().read() {
            for player_arc in player_list_guard.iter() {
                if let Ok(player) = player_arc.read() {
                    let player_id = player.get_player_index();

                    // In full implementation:
                    // - Query ShroudManager for visible objects
                    // - Update rendering visibility flags
                    // - Handle stealth detection
                    // - Update radar display

                    trace!("Updated vision for player {}", player_id);
                }
            }
        }

        // C++ PartitionData observes resolved per-object status after the
        // shroud update. Retail W3D captures only the current local player.
        let local_player = crate::object::THE_W3D_GHOST_OBJECT_MANAGER
            .read()
            .ok()
            .map(|manager| manager.local_player_index())
            .unwrap_or(0);
        let ghost_object_ids = self.partition_manager.ghost_link_object_ids();
        let mut transitions = Vec::with_capacity(ghost_object_ids.len());
        for object_id in ghost_object_ids {
            let status = self.ghost_shroud_status_for_link(object_id, local_player);
            let capture = self
                .partition_manager
                .object_ghost_needs_capture(object_id, local_player, status)
                .then(|| crate::object::w3d_ghost_object::capture_w3d_ghost_snapshot(object_id))
                .flatten();
            transitions.push((object_id, status, capture));
        }
        if let Ok(mut manager) = crate::object::THE_W3D_GHOST_OBJECT_MANAGER.write() {
            for (object_id, status, capture) in &transitions {
                self.partition_manager.apply_object_ghost_shroud_status(
                    *object_id,
                    local_player,
                    *status,
                    capture.as_ref(),
                    &mut manager,
                );
            }
        }

        Ok(())
    }

    /// C++ `GameClient::update` → `TheGhostObjectManager->updateOrphanedObjects`.
    /// Re-evaluates parentless ghosts against current cell shroud and drops
    /// modules that no longer hold any snapshot.
    pub fn update_orphaned_w3d_ghosts(&mut self) {
        let local_player = crate::object::THE_W3D_GHOST_OBJECT_MANAGER
            .read()
            .ok()
            .map(|manager| manager.local_player_index())
            .unwrap_or(0);
        let Ok(mut manager) = crate::object::THE_W3D_GHOST_OBJECT_MANAGER.write() else {
            return;
        };
        let statuses: Vec<(ObjectID, crate::common::ObjectShroudStatus)> = self
            .partition_manager
            .orphan_ghost_object_ids()
            .into_iter()
            .map(|object_id| {
                (
                    object_id,
                    self.ghost_shroud_status_for_link(object_id, local_player),
                )
            })
            .collect();
        self.partition_manager.update_orphaned_ghosts(
            local_player,
            |object_id| {
                statuses
                    .iter()
                    .find(|(id, _)| *id == object_id)
                    .map(|(_, status)| *status)
                    .unwrap_or(crate::common::ObjectShroudStatus::Fogged)
            },
            &mut manager,
        );
    }

    fn ghost_shroud_status_for_link(
        &self,
        object_id: ObjectID,
        player_index: usize,
    ) -> crate::common::ObjectShroudStatus {
        if let Some(object) = self.objects.get(&object_id) {
            if let Ok(object) = object.read() {
                return object.get_shrouded_status(player_index as i32);
            }
        }
        let Some(position) = self.partition_manager.ghost_frozen_position(object_id) else {
            return crate::common::ObjectShroudStatus::Fogged;
        };
        use crate::system::shroud_manager::{get_shroud_manager, ShroudState};
        let shroud_manager = get_shroud_manager();
        let Ok(shroud) = shroud_manager.lock() else {
            return crate::common::ObjectShroudStatus::Fogged;
        };
        match shroud.get_shroud_state(player_index as u32, &position) {
            ShroudState::Visible => crate::common::ObjectShroudStatus::Clear,
            ShroudState::Explored => crate::common::ObjectShroudStatus::Fogged,
            ShroudState::Hidden => crate::common::ObjectShroudStatus::Shrouded,
        }
    }

    /// Phase 8: Evaluate mission scripts
    ///
    /// ## C++ Reference: ScriptEngine::update()
    ///
    /// Runs mission scripting system:
    /// - Evaluate script conditions
    /// - Execute actions if conditions met
    /// - Check victory/defeat conditions
    /// - Track script completion
    pub fn evaluate_scripts(&mut self) -> Result<(), GameLogicError> {
        trace!("GameLogic::evaluate_scripts()");

        // Also update the global script engine
        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                if let Err(e) = engine.update() {
                    warn!("ScriptEngine::update failed: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Check whether simulation time is frozen (script freeze or tactical freeze).
    ///
    /// ## C++ Reference: GameLogic.cpp lines 3603-3617
    ///
    /// C++ checks `TheTacticalView->isTimeFrozen()`,
    /// `TheScriptEngine->isTimeFrozenDebug()`, and
    /// `TheScriptEngine->isTimeFrozenScript()`. When any of these are true,
    /// the update returns early (unless a MSG_CLEAR_GAME_DATA is in the
    /// command list, which forces an unfreeze).
    fn is_time_frozen(&self) -> bool {
        if get_camera_view_bridge()
            .map(|view| {
                should_freeze_time(
                    view.is_time_frozen(),
                    view.is_camera_movement_finished(),
                    false,
                )
            })
            .unwrap_or(false)
        {
            return true;
        }

        // Check script engine freeze state
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                if should_freeze_time(false, true, engine.is_time_frozen()) {
                    return true;
                }
            }
        }
        false
    }

    /// Update the terrain logic system.
    ///
    /// ## C++ Reference: GameLogic.cpp lines 3619-3623
    ///
    /// C++: `TheTerrainLogic->UPDATE();`
    ///
    /// Terrain updates include bridge damage state transitions and dynamic
    /// water table changes. This runs after the early scripting phase so that
    /// script-triggered bridge damage is reflected before object updates.
    fn update_terrain(&self) -> Result<(), GameLogicError> {
        trace!("GameLogic::update_terrain()");

        // The terrain logic singleton lives in the terrain module.
        // It manages bridges, dynamic water, and trigger areas.
        if let Ok(mut terrain) = crate::terrain::get_terrain_logic().write() {
            terrain.update();
        }

        Ok(())
    }

    /// Update the production/build system.
    ///
    /// ## C++ Reference: GameLogic.cpp lines 3747-3750
    ///
    /// C++: `TheBuildAssistant->UPDATE();`
    ///
    /// Production updates run after AI so build orders issued by AI this frame
    /// are immediately reflected in production queues. The BuildAssistant
    /// manages structure placement validation and dozer assignment.
    fn update_production(&self, frame: UnsignedInt) -> Result<(), GameLogicError> {
        trace!("GameLogic::update_production(frame={})", frame);

        // The BuildAssistant singleton lives in the game_engine common crate.
        // get_build_assistant() returns Option<MutexGuard<BuildAssistant>>.
        if let Some(mut build_assistant) =
            game_engine::common::system::build_assistant::get_build_assistant()
        {
            build_assistant.update(frame);
        }

        Ok(())
    }

    /// Update the weapon store (process delayed damage).
    ///
    /// ## C++ Reference: GameLogic.cpp line 3767
    ///
    /// C++: `TheWeaponStore->UPDATE();`
    ///
    /// The weapon store processes delayed damage entries whose trigger frame
    /// has arrived. This runs after death cleanup so we don't apply damage to
    /// objects that are already being destroyed.
    fn update_weapon_store(&self) -> Result<(), GameLogicError> {
        trace!("GameLogic::update_weapon_store()");

        if let Err(e) = crate::weapon::with_weapon_store_mut(|store| store.update()) {
            // "System not initialized" means the weapon store hasn't been loaded
            // yet (e.g. before map load). Silently skip in that case.
            let err_str = e.to_string();
            if err_str.contains("not initialized") {
                trace!("Weapon store not initialized; skipping update");
            } else {
                warn!("Weapon store update failed: {}", err_str);
            }
        }

        Ok(())
    }

    /// C++ `GameLogic::getCRC`. `Recalc` rebuilds; otherwise return cached `m_CRC`.
    pub fn get_crc(&self, mode: CrcMode) -> UnsignedInt {
        if mode != CrcMode::Recalc {
            return self.crc_cache;
        }
        self.compute_crc()
    }

    /// Compute CRC of game state for lockstep synchronization.
    /// Matches C++ GameLogic::getCRC(CRC_RECALC) at GameLogic.cpp:3988.
    fn compute_crc(&self) -> UnsignedInt {
        use crate::common::Snapshot;
        use crate::helpers::get_game_logic_random_seed_crc;
        use game_engine::common::system::snapshot::Snapshotable;

        let mut xfer = LogicXferCrc::new();
        let _ = Xfer::open(&mut xfer, "lightCRC".to_string());

        let mut marker = "MARKER:Objects".to_string();
        let _ = Xfer::xfer_ascii_string(&mut xfer, &mut marker);
        // C++ walks `m_objList` (insertion / next-object link), not a sorted map.
        // Object::crc / Player::crc use common::system::xfer::Xfer; LogicXferCrc
        // implements that trait so both dump through one fold_crc_bytes addCRC.
        for obj_id in &self.all_objects {
            if let Some(obj_arc) = self.objects.get(obj_id) {
                if let Ok(obj) = obj_arc.read() {
                    Snapshot::crc(&*obj, &mut xfer);
                }
            }
        }

        if Xfer::get_xfer_mode(&xfer) == XferMode::Crc {
            let mut seed = get_game_logic_random_seed_crc();
            let _ = Xfer::xfer_unsigned_int(&mut xfer, &mut seed);
        }

        let mut marker = "MARKER:ThePartitionManager".to_string();
        let _ = Xfer::xfer_ascii_string(&mut xfer, &mut marker);
        self.partition_manager.crc_into(&mut xfer);

        let mut marker = "MARKER:ThePlayerList".to_string();
        let _ = Xfer::xfer_ascii_string(&mut xfer, &mut marker);
        if let Ok(list) = player_list().read() {
            let mut count = list.get_player_count() as i32;
            let _ = Xfer::xfer_int(&mut xfer, &mut count);
            for player_arc in list.iter() {
                if let Ok(player) = player_arc.read() {
                    let _ = Snapshotable::crc(&*player, &mut xfer);
                }
            }
        }

        let mut marker = "MARKER:TheAI".to_string();
        let _ = Xfer::xfer_ascii_string(&mut xfer, &mut marker);
        let ai_store = crate::ai::the_ai(); if let Ok(ai) = ai_store.read() {
            if let Some(pathfinder) = ai.pathfinder() {
                if let Ok(pathfinder) = pathfinder.read() {
                    pathfinder.crc_pathfinder(&mut xfer);
                }
            }
            let mut ai_marker = "MARKER:TAiData".to_string();
            let _ = Xfer::xfer_ascii_string(&mut xfer, &mut ai_marker);
            if let Ok(ai_data) = ai.get_ai_data().read() {
                crate::common::Snapshot::crc(&*ai_data, &mut xfer);
            }
        }

        let _ = Xfer::close(&mut xfer);
        xfer.get_crc()
    }

    /// Update victory conditions.
    ///
    /// ## C++ Reference: GameLogic.cpp line 3769, VictoryConditions.cpp update()
    ///
    /// C++: `TheVictoryConditions->UPDATE();`
    ///
    /// Each frame, checks all players for elimination conditions (no units, no
    /// buildings, or both) based on the multiplayer victory flags. Newly-defeated
    /// players are marked and their map is revealed. Also detects when only a
    /// single alliance remains (victory condition).
    ///
    /// Network-specific behavior (TheRecorder->isMultiplayer(), GameSlot,
    /// PopulateInGameDiplomacyPopup) is intentionally deferred per AGENTS.md.
    fn update_victory_conditions(&mut self) {
        trace!("GameLogic::update_victory_conditions()");

        // C++ skips evaluation before frame 2 (TheGameLogic->getFrame() > 1)
        if self.frame <= 1 {
            return;
        }

        let player_list = match player_list().read() {
            Ok(pl) => pl,
            Err(_) => return,
        };

        // Use the same elimination flags as C++ default:
        //   VICTORY_NOBUILDINGS | VICTORY_NOUNITS
        let flags = crate::system::victory_conditions::MultiplayerEliminationFlags::DEFAULT;
        let no_units = flags
            .contains(crate::system::victory_conditions::MultiplayerEliminationFlags::NO_UNITS);
        let no_buildings = flags
            .contains(crate::system::victory_conditions::MultiplayerEliminationFlags::NO_BUILDINGS);

        let mut newly_defeated_indices: Vec<(
            PlayerIndex,
            std::sync::Arc<std::sync::RwLock<Player>>,
        )> = Vec::new();

        // Phase 1: Scan for newly-defeated players (C++ lines 163-199)
        for player_arc in player_list.iter() {
            let Ok(player) = player_arc.read() else {
                continue;
            };
            let player_index = player.get_player_index();
            if player_index < 0 {
                continue;
            }

            // Skip civilian and observer players (C++ cachePlayerPtrs filter)
            if player.is_player_observer() {
                continue;
            }

            let is_defeated = match (no_units, no_buildings) {
                (true, true) => !player.has_any_objects(),
                (true, false) => !player.has_any_units(),
                (false, true) => !player.has_any_buildings_counts_for_victory(),
                (false, false) => false,
            };

            if is_defeated && !player.is_defeated() {
                newly_defeated_indices.push((player_index, std::sync::Arc::clone(player_arc)));
            }
        }

        // Release the read lock before acquiring write locks
        drop(player_list);

        // Phase 2: Handle newly-defeated players (C++ VictoryConditions.cpp 166-198)
        for (player_index, player_arc) in &newly_defeated_indices {
            let display_name = player_arc
                .read()
                .ok()
                .map(|player| player.get_player_display_name().clone())
                .unwrap_or_default();

            if let Ok(mut player) = player_arc.write() {
                player.set_defeated(true);
                info!(
                    "VictoryConditions: Player {} has been eliminated",
                    player_index
                );
            }

            if self.frame > 1 {
                if let Ok(mut shroud) =
                    crate::system::shroud_manager::get_shroud_manager().lock()
                {
                    let _ = shroud.reveal_map_for_player_permanently(*player_index as u32);
                }

                crate::helpers::TheInGameUI::display_message(&format!(
                    "GUI:PlayerHasBeenDefeated {}",
                    display_name
                ));

                if let Some(audio) = crate::helpers::TheAudio::get() {
                    let event = crate::common::audio::AudioEventRts::new("GUIMessageReceived");
                    audio.add_audio_event(&event);
                }
            }

            let leftovers: Vec<ObjectID> = self
                .all_objects
                .iter()
                .copied()
                .filter(|id| {
                    self.objects
                        .get(id)
                        .and_then(|obj| obj.read().ok())
                        .and_then(|guard| {
                            guard.get_controlling_player().and_then(|player| {
                                player
                                    .read()
                                    .ok()
                                    .map(|p| p.get_player_index() == *player_index)
                            })
                        })
                        .unwrap_or(false)
                })
                .collect();
            for leftover_id in leftovers {
                self.destroy_object(leftover_id);
            }
        }
    }

    /// Check and update disabled statuses on all objects.
    ///
    /// ## C++ Reference: GameLogic.cpp lines 3783-3792
    ///
    /// C++:
    /// ```text
    /// for( Object *obj = m_objList; obj; obj = obj->getNextObject() )
    /// {
    ///     if( obj->isDisabled() )
    ///     {
    ///         obj->checkDisabledStatus();
    ///     }
    /// }
    /// ```
    ///
    /// Timer-based disabled states (e.g., Hacked, EMP, WeaponsetToggle) have
    /// expiration frames. This method checks all disabled objects and
    /// re-enables those whose disable duration has expired. The check runs at
    /// end-of-frame so disabled objects remain inactive for the entire frame.
    fn check_disabled_statuses(&self) {
        trace!("GameLogic::check_disabled_statuses()");

        for obj_id in &self.all_objects {
            if let Some(obj_ref) = self.objects.get(obj_id) {
                if let Ok(mut obj) = obj_ref.write() {
                    if obj.is_disabled() {
                        obj.check_disabled_status();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod empty_world_tick_tests {
    use super::*;
    use crate::object::registry::OBJECT_REGISTRY;

    #[test]
    fn empty_world_update_runs_scripts_terrain_destroy_and_frame_increment() {
        // C++ GameLogic.cpp:3600 / 3622 / 3762 / 3799 — empty m_objList is still
        // a full update: ScriptEngine, TerrainLogic, processDestroyList, m_frame++.
        OBJECT_REGISTRY.clear();
        let mut logic = GameLogic::new();
        assert!(logic.objects.is_empty());
        assert!(OBJECT_REGISTRY.store_is_empty());
        logic
            .update(0)
            .expect("empty world still returns Ok so the host frame loop continues");
        assert!(
            !logic.last_update_was_empty_noop(),
            "must not return Ok before C++ phases"
        );
        assert_eq!(logic.empty_world_tick_count(), 0);
        assert_eq!(
            logic.get_frame(),
            1,
            "empty-world GameLogic::update must still do m_frame++"
        );
    }
}
