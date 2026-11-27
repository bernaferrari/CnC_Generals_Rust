impl GameLogic {
    /// Transfer the native snapshot using the same implementation as the
    /// registered bridge. Errors do not roll back earlier mutations or construction
    /// side effects; this is not the host WorldSnapshot format.
    pub fn xfer_native_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        // Without a recipient, no load may mutate the world before discovering
        // an active Campaign chunk. Explicit standalone readers pass an empty DTO.
        if xfer.get_xfer_mode() == XferMode::Load
            || self.campaign_session_requires_recipient
            || self.game_mode == GAME_SINGLE_PLAYER
        {
            return Err(XferStatus::InvalidData);
        }
        self.xfer_native_snapshot_with_campaign(xfer, &mut Default::default())
    }

    /// Borrow the serializer's canonical campaign DTO for the original v5 byte codec.
    /// Native Main uses its separate WorldSnapshot serializer with the same owner DTO.
    pub fn xfer_native_snapshot_with_campaign(
        &mut self,
        xfer: &mut dyn Xfer,
        campaign: &mut game_engine::System::CampaignManagerXferState,
    ) -> Result<(), XferStatus> {
        xfer_game_logic_state_with_campaign(self, xfer, campaign)?;
        self.admit_campaign_context(campaign);
        Ok(())
    }

    pub fn admit_campaign_context(
        &mut self,
        campaign: &game_engine::System::CampaignManagerXferState,
    ) {
        self.campaign_session_is_challenge = campaign.is_challenge;
        self.campaign_session_requires_recipient = campaign.is_challenge
            || !campaign.campaign.is_empty()
            || !campaign.mission.is_empty()
            || campaign.challenge_info.is_some();
    }

    /// GameLogic.cpp:4584–4651 removes map actors in object-list order,
    /// queuing each bridge's towers immediately before that bridge.
    fn prepare_logic_for_object_load(&mut self) -> Result<(), XferStatus> {
        let terrain = self
            .owning_world_services()
            .map(|world| Arc::clone(world.terrain()))
            .unwrap_or_else(crate::terrain::get_terrain_logic);
        let terrain = terrain.read().map_err(|_| XferStatus::InvalidData)?;
        let mut ids_to_destroy = Vec::new();
        for &id in &self.all_objects {
            let object = self.objects.get(&id).ok_or(XferStatus::InvalidData)?;
            let object = object.read().map_err(|_| XferStatus::InvalidData)?;
            if object.is_kind_of(KindOf::Bridge) {
                let bridge = terrain
                    .find_bridge_at(object.get_position())
                    .map(|bridge| bridge.get_bridge_info())
                    .filter(|info| info.bridge_object_id == id)
                    .ok_or_else(|| {
                        warn!("Core Object load could not find map metadata for bridge {id}");
                        XferStatus::InvalidData
                    })?;
                ids_to_destroy.extend(
                    bridge
                        .tower_object_id
                        .iter()
                        .copied()
                        .filter(|tower| *tower != INVALID_ID),
                );
                ids_to_destroy.push(id);
            } else if object.is_kind_of(KindOf::WalkOnTopOfWall) {
                ids_to_destroy.push(id);
            }
        }
        // Destruction can update the same terrain and pathfinding services.
        drop(terrain);
        for obj_id in ids_to_destroy {
            self.destroy_object(obj_id);
        }
        self.process_destroy_list().map_err(|error| {
            warn!("Core Object load could not retire map actors: {error:?}");
            XferStatus::InvalidData
        })
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
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, command_button_name.map(str::to_string));
    }

    /// PARITY_NOTE: GameLogic::findControlBarOverride(AsciiString, Int, ConstCommandButtonPtr&) C++ line 4398.
    pub fn find_control_bar_override(
        &self,
        command_set_name: &str,
        slot: i32,
    ) -> Option<Option<String>> {
        let key = control_bar_override_key(command_set_name, slot)?;
        self.control_bar_overrides
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .cloned()
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

    pub fn snapshot_buildable_status_overrides(&self) -> Vec<(String, Int)> {
        let mut entries: Vec<(String, Int)> = self
            .buildable_status_overrides
            .iter()
            .map(|(name, status)| (name.clone(), *status))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
    }

    pub fn restore_buildable_status_overrides(&mut self, entries: Vec<(String, Int)>) {
        self.buildable_status_overrides = entries.into_iter().collect();
    }

    pub fn snapshot_control_bar_overrides_raw(&self) -> Vec<(String, Option<String>)> {
        let mut entries: Vec<(String, Option<String>)> = self
            .control_bar_overrides
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
    }

    pub fn restore_control_bar_overrides_raw(&mut self, entries: Vec<(String, Option<String>)>) {
        *self.control_bar_overrides.write().unwrap_or_else(|e| e.into_inner()) =
            entries.into_iter().collect();
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

    /// Run the nested Team fixups against this Object table once every block
    /// has transferred, then remake the IDs and heap (GameLogic.cpp:4996–5071).
    pub fn load_post_process(&mut self) -> Result<(), XferStatus> {
        self.validate_restored_team_members()?;
        self.next_object_id = INVALID_ID;
        for obj_id in &self.all_objects {
            if *obj_id >= self.next_object_id {
                self.next_object_id = obj_id.saturating_add(1);
            }
        }
        if self.next_object_id == INVALID_ID {
            self.next_object_id = 1;
        }
        self.rebuild_sleepy_updates_after_load();
        Ok(())
    }

    fn validate_restored_team_members(&self) -> Result<(), XferStatus> {
        let Some(world) = self.owning_world_services() else {
            return Ok(());
        };
        let teams = world
            .team_factory()
            .with_read_without_flushing(crate::team::TeamFactory::get_all_teams)
            .map_err(|_| XferStatus::InvalidData)?;
        for team in teams {
            let mut roster = team.try_write().map_err(|_| XferStatus::InvalidData)?;
            roster
                .validate_restored_members(|id| {
                    self.objects
                        .get(&id)
                        .and_then(|object| object.try_read().ok())
                        .is_some_and(|object| object.is_admitted_to_team(&team))
                })
                .map_err(|error| {
                    warn!("Core team snapshot: {error}");
                    XferStatus::InvalidData
                })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod map_object_load_cleanup_tests {
    use super::*;
    use crate::common::DefaultThingTemplate;
    use crate::path::PathfindLayerEnum;
    use crate::system::engine_stores::{EngineStores, with_active_stores};
    use crate::terrain::TerrainLogic;

    #[test]
    fn core_load_clears_receiving_map_actors_in_object_list_order() {
        let receiving = Arc::new(EngineStores::new_for_world());
        let peer = Arc::new(EngineStores::new_for_world());
        let north = Coord3D::new(350.0, 400.0, 0.0);
        let south = Coord3D::new(350.0, 500.0, 0.0);
        let north_bridge = 91_210;
        let south_bridge = 91_220;
        let north_tower = 91_211;
        let south_tower = 91_221;
        let wall = 91_230;
        let unrelated = 91_240;
        let add_bridge = |world: &EngineStores, position, bridge, tower| {
            let mut info = generals_sim::terrain::BridgeInfo::from_landmark(position, 0.0, 50.0, 20.0, bridge);
            info.tower_object_id[0] = tower;
            world
                .services()
                .terrain()
                .write()
                .unwrap()
                .prepend_bridge_on_layer(info, "CoreLoadBridge".into(), PathfindLayerEnum::Ground);
        };
        add_bridge(&receiving, north, north_bridge, north_tower);
        add_bridge(&receiving, south, south_bridge, south_tower);
        add_bridge(&peer, north, north_bridge, unrelated);
        add_bridge(&peer, south, south_bridge, unrelated);

        let mut logic = GameLogic::new_with_engine_stores(receiving.clone());
        with_active_stores(&receiving, || {
            // GameLogic prepends, so reverse this fixture's desired walk order.
            for (id, kind, position) in [
                (north_bridge, KindOf::Bridge, north),
                (wall, KindOf::WalkOnTopOfWall, Coord3D::ZERO),
                (south_bridge, KindOf::Bridge, south),
                (north_tower, KindOf::Inert, north),
                (south_tower, KindOf::Inert, south),
                (unrelated, KindOf::Inert, Coord3D::ZERO),
            ]
            .into_iter()
            .rev()
            {
                let mut template = DefaultThingTemplate::new(format!("MapActor{id}"));
                template.add_kind_of(KindOf::Inert);
                template.add_kind_of(kind);
                let mut object =
                    Object::new_raw(Arc::new(template), id, ObjectStatusMaskType::none(), None);
                object.set_position(&position).unwrap();
                logic
                    .register_object(Arc::new(RwLock::new(object)))
                    .unwrap();
            }
        });

        with_active_stores(&peer, || logic.prepare_logic_for_object_load()).unwrap();
        let destroyed: Vec<_> = logic
            .event_queue
            .iter()
            .filter_map(|event| match event {
                GameEvent::ObjectDestroyed(id) => Some(*id),
                _ => None,
            })
            .collect();
        assert_eq!(
            destroyed,
            [north_tower, north_bridge, wall, south_tower, south_bridge]
        );
        assert_eq!(logic.iter_all_object_ids().collect::<Vec<_>>(), [unrelated]);
        with_active_stores(&receiving, || logic.clear_all_objects());
    }
}
