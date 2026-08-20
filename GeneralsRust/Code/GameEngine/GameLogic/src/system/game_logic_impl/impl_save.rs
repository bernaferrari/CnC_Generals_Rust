impl GameLogic {
    pub fn prepare_logic_for_object_load(&mut self) {
        let bridge_towers_to_destroy: Vec<ObjectID> = {
            let terrain = crate::terrain::get_terrain_logic();
            let terrain_guard = match terrain.read() {
                Ok(g) => g,
                Err(_) => return,
            };
            let mut towers = Vec::new();
            for &obj_id in &self.all_objects {
                let Some(obj_ref) = self.objects.get(&obj_id) else {
                    continue;
                };
                let Ok(obj) = obj_ref.read() else { continue };
                if obj.is_kind_of(KindOf::Bridge) {
                    let pos = *obj.get_position();
                    if let Some(bridge) = terrain_guard.find_bridge_at(&pos) {
                        for &tower_id in &bridge.get_bridge_info().tower_object_id {
                            if tower_id != INVALID_ID {
                                towers.push(tower_id);
                            }
                        }
                    }
                }
            }
            towers
        };

        for tower_id in bridge_towers_to_destroy {
            self.destroy_object(tower_id);
        }

        let ids_to_destroy: Vec<ObjectID> = self
            .all_objects
            .iter()
            .filter(|&&obj_id| {
                if let Some(obj_ref) = self.objects.get(&obj_id) {
                    if let Ok(obj) = obj_ref.read() {
                        return obj.is_kind_of(KindOf::Bridge)
                            || obj.is_kind_of(KindOf::WalkOnTopOfWall);
                    }
                }
                false
            })
            .copied()
            .collect();

        for obj_id in ids_to_destroy {
            self.destroy_object(obj_id);
        }
        let _ = self.process_destroy_list();
    }

    // =========================================================================
    // C++ Parity: ControlBar overrides
    // =========================================================================

    /// PARITY_NOTE: GameLogic::setControlBarOverride(AsciiString, Int, ConstCommandButtonPtr) C++ line 4389.
    pub fn set_control_bar_override(
        &mut self,
        command_set_name: &str,
        slot: i32,
        command_button_name: Option<&str>,
    ) {
        let Some(key) = control_bar_override_key(command_set_name, slot) else {
            return;
        };
        self.control_bar_overrides
            .insert(key, command_button_name.map(str::to_string));
    }

    /// PARITY_NOTE: GameLogic::findControlBarOverride(AsciiString, Int, ConstCommandButtonPtr&) C++ line 4398.
    pub fn find_control_bar_override(
        &self,
        command_set_name: &str,
        slot: i32,
    ) -> Option<Option<&str>> {
        let key = control_bar_override_key(command_set_name, slot)?;
        self.control_bar_overrides
            .get(&key)
            .map(|value| value.as_deref())
    }

    // =========================================================================
    // C++ Parity: Superweapon restrictions
    // =========================================================================

    pub fn get_superweapon_restriction(&self) -> UnsignedShort {
        self.superweapon_restriction
    }

    pub fn set_superweapon_restriction(&mut self, restriction: UnsignedShort) {
        self.superweapon_restriction = restriction;
    }

    // =========================================================================
    // C++ Parity: Object TOC for save/load
    // =========================================================================

    /// PARITY_NOTE: GameLogic::findTOCEntryByName(AsciiString) C++ line 4460.
    pub fn find_toc_entry_by_name(&self, name: &str) -> Option<&ObjectTOCEntry> {
        self.object_toc.iter().find(|e| e.name == name)
    }

    /// PARITY_NOTE: GameLogic::findTOCEntryById(UnsignedShort) C++ line 4474.
    pub fn find_toc_entry_by_id(&self, id: UnsignedShort) -> Option<&ObjectTOCEntry> {
        self.object_toc.iter().find(|e| e.id == id)
    }

    /// PARITY_NOTE: GameLogic::addTOCEntry(AsciiString, UnsignedShort) C++ line 4488.
    pub fn add_toc_entry(&mut self, name: String, id: UnsignedShort) {
        self.object_toc.push(ObjectTOCEntry { name, id });
    }

    /// PARITY_NOTE: GameLogic::xferObjectTOC(Xfer*) C++ line 4501.
    /// Serializes/deserializes the object TOC used during save/load.
    pub fn xfer_object_toc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        self.object_toc.clear();
        let mut toc_count: UnsignedInt = 0;
        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                // PARITY_NOTE: Collect unique template names first to avoid
                // borrowing self mutably while iterating. C++ uses a plain
                // loop with direct map insertion (no borrow checker).
                let mut seen_names: std::collections::HashSet<String> =
                    self.object_toc.iter().map(|e| e.name.clone()).collect();
                let mut new_entries: Vec<(String, UnsignedShort)> = Vec::new();
                for obj_id in &self.all_objects {
                    if let Some(obj_ref) = self.objects.get(obj_id) {
                        if let Ok(obj) = obj_ref.read() {
                            let tname = obj.get_template().get_name().to_string();
                            if !seen_names.contains(&tname) {
                                seen_names.insert(tname.clone());
                                toc_count += 1;
                                new_entries.push((tname, toc_count as UnsignedShort));
                            }
                        }
                    }
                }
                for (name, id) in new_entries {
                    self.add_toc_entry(name, id);
                }
                xfer.xfer_unsigned_int(&mut toc_count)?;
                for entry in &mut self.object_toc {
                    xfer.xfer_string(&mut entry.name)?;
                    xfer.xfer_unsigned_short(&mut entry.id)?;
                }
            }
            XferMode::Load => {
                xfer.xfer_unsigned_int(&mut toc_count)?;
                for _ in 0..toc_count {
                    let mut name_str = String::new();
                    let mut id: UnsignedShort = 0;
                    xfer.xfer_string(&mut name_str)?;
                    xfer.xfer_unsigned_short(&mut id)?;
                    self.add_toc_entry(name_str, id);
                }
            }
            XferMode::Invalid => return Err(XferStatus::ModeUnknown),
        }
        Ok(())
    }

    pub fn is_game_paused(&self) -> bool {
        self.game_paused
    }

    /// PARITY_NOTE: GameLogic::setGamePaused(Bool, Bool) C++ line 4164.
    pub fn set_game_paused(&mut self, paused: bool, _pause_music: bool) {
        if paused == self.game_paused {
            return;
        }
        self.game_paused = paused;
    }

    /// C++ `GameLogic::loadPostProcess` (GameLogic.cpp:4996-5071) remakes IDs and the
    /// sleepy heap only. `ThePartitionManager->update()` is `GameState::gameStatePostProcessLoad`
    /// after every snapshot (GameState.cpp:1528-1529), not here.
    pub fn load_post_process(&mut self) {
        self.next_object_id = INVALID_ID;
        for obj_id in &self.all_objects {
            if *obj_id >= self.next_object_id {
                self.next_object_id = obj_id.saturating_add(1);
            }
        }
        if self.next_object_id == INVALID_ID {
            self.next_object_id = 1;
        }
        self.sleepy_updates.clear();
        self.normal_updates.clear();
        self.module_lookup.clear();

        // C++ walks every object's UpdateModules and re-pushes the sleepy heap.
        let now = if self.frame == 0 { 1 } else { self.frame };
        let object_ids: Vec<ObjectID> = self.all_objects.clone();
        for obj_id in object_ids {
            let Some(arc) = self.find_object_by_id(obj_id) else {
                continue;
            };
            let Ok(obj) = arc.read() else {
                continue;
            };
            for module in obj.update_module_registrations() {
                self.register_sleepy_update_module(obj_id, module.clone(), now);
            }
        }
        self.remake_sleepy_update();
    }

}
