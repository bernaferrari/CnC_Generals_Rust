use super::*;

impl Player {
    // =========================================================
    // AI System Integration (C++ Player.cpp lines 695-712)
    // =========================================================

    /// Set the AI player reference
    /// C++ Reference: Player::setPlayerType() creates and assigns m_ai
    pub fn set_ai(&mut self, ai: Option<Arc<dyn AIPlayerInterface>>) {
        self.ai = ai.map(|arc| Arc::downgrade(&arc));
    }

    /// Get the AI player reference
    /// Returns None if player is human or AI has been destroyed
    pub fn get_ai(&self) -> Option<Arc<dyn AIPlayerInterface>> {
        self.ai.as_ref().and_then(|weak| weak.upgrade())
    }

    /// Check if this player has an AI controller
    /// C++ Reference: m_ai != NULL checks throughout Player.cpp
    pub fn has_ai(&self) -> bool {
        self.ai
            .as_ref()
            .map_or(false, |weak| weak.strong_count() > 0)
    }

    /// Get player difficulty
    /// C++ Reference: Player::getPlayerDifficulty() (Player.cpp lines 1500-1505)
    pub fn get_player_difficulty(&self) -> GameDifficulty {
        self.difficulty
    }

    /// Set player difficulty
    pub fn set_player_difficulty(&mut self, difficulty: GameDifficulty) {
        self.difficulty = difficulty;
        if let Some(ai) = self.get_ai() {
            ai.set_ai_difficulty(difficulty);
        }
    }

    /// Check if this is a skirmish AI player
    /// C++ Reference: Player::isSkirmishAIPlayer() (Player.cpp lines 1491-1494)
    pub fn is_skirmish_ai_player(&self) -> bool {
        self.get_ai().map_or(false, |ai| ai.is_skirmish_ai())
    }

    /// Get current enemy for AI
    /// C++ Reference: Player::getCurrentEnemy() (Player.cpp lines 1486-1489)
    pub fn get_current_enemy(&self) -> Option<i32> {
        self.get_ai().and_then(|ai| ai.get_ai_enemy())
    }

    // =========================================================
    // Build List Management (C++ Player.cpp lines 592-636)
    // =========================================================

    /// Set the build list
    /// C++ Reference: Player::setBuildList() (Player.cpp lines 592-598)
    pub fn set_build_list(&mut self, build_list: Option<Box<BuildListInfo>>) {
        self.build_list = build_list;
    }

    /// Get the build list
    /// C++ Reference: Player::getBuildList() (Player.h line 316)
    pub fn get_build_list(&self) -> Option<&BuildListInfo> {
        self.build_list.as_deref()
    }

    /// Get mutable build list
    pub fn get_build_list_mut(&mut self) -> Option<&mut BuildListInfo> {
        self.build_list.as_deref_mut()
    }

    /// Add an object to the build list
    /// C++ Reference: Player::addToBuildList() (Player.cpp lines 601-610)
    pub fn add_to_build_list(
        &mut self,
        object_id: ObjectID,
        template_name: String,
        location: Coord3D,
        angle: f32,
    ) {
        let mut new_info = Box::new(BuildListInfo::new(template_name, location, angle));
        new_info.set_object_id(object_id);
        new_info.set_num_rebuilds(0); // Can't rebuild
        new_info.set_next(self.build_list.take());
        self.build_list = Some(new_info);
    }

    /// Add to priority build list
    /// C++ Reference: Player::addToPriorityBuildList() (Player.cpp lines 613-623)
    pub fn add_to_priority_build_list(
        &mut self,
        template_name: String,
        location: Coord3D,
        angle: f32,
    ) {
        let mut new_info = Box::new(BuildListInfo::new(template_name, location, angle));
        new_info.mark_priority_build();
        new_info.set_num_rebuilds(1); // Build once
        new_info.set_next(self.build_list.take());
        self.build_list = Some(new_info);
    }

    // =========================================================
    // Resource Gathering Manager (C++ ResourceGatheringManager.h)
    // =========================================================

    /// Add a supply center
    /// C++ Reference: ResourceGatheringManager::addSupplyCenter()
    pub fn add_supply_center(&mut self, center_id: ObjectID) {
        self.supply_centers.push(center_id);
    }

    /// Remove a supply center
    /// C++ Reference: ResourceGatheringManager::removeSupplyCenter()
    pub fn remove_supply_center(&mut self, center_id: ObjectID) {
        self.supply_centers.retain(|&id| id != center_id);
    }

    /// Add a supply warehouse
    /// C++ Reference: ResourceGatheringManager::addSupplyWarehouse()
    pub fn add_supply_warehouse(&mut self, warehouse_id: ObjectID) {
        self.supply_warehouses.push(warehouse_id);
    }

    /// Remove a supply warehouse
    /// C++ Reference: ResourceGatheringManager::removeSupplyWarehouse()
    pub fn remove_supply_warehouse(&mut self, warehouse_id: ObjectID) {
        self.supply_warehouses.retain(|&id| id != warehouse_id);
    }

    /// Get all supply centers
    pub fn get_supply_centers(&self) -> &[ObjectID] {
        &self.supply_centers
    }

    /// Get all supply warehouses
    pub fn get_supply_warehouses(&self) -> &[ObjectID] {
        &self.supply_warehouses
    }

    /// Find best supply warehouse for a query object
    /// C++ Reference: ResourceGatheringManager::findBestSupplyWarehouse()
    pub fn find_best_supply_warehouse(&self, _query_object_id: ObjectID) -> Option<ObjectID> {
        self.supply_warehouses.first().copied()
    }

    /// Find best supply warehouse using world state for C++ ResourceGatheringManager parity.
    pub fn find_best_supply_warehouse_with_world<W: ResourceWorld>(
        &mut self,
        query_object_id: ObjectID,
        world: &W,
    ) -> Option<ObjectID> {
        let mut manager = self.resource_manager_from_player_lists();
        let best = manager.find_best_supply_warehouse(query_object_id, world);
        self.sync_supply_lists_from_manager(&manager);
        best
    }

    /// Find best supply center for a query object
    /// C++ Reference: ResourceGatheringManager::findBestSupplyCenter()
    pub fn find_best_supply_center(&self, _query_object_id: ObjectID) -> Option<ObjectID> {
        self.supply_centers.first().copied()
    }

    /// Find best supply center using world state for C++ ResourceGatheringManager parity.
    pub fn find_best_supply_center_with_world<W: ResourceWorld>(
        &mut self,
        query_object_id: ObjectID,
        world: &W,
    ) -> Option<ObjectID> {
        let mut manager = self.resource_manager_from_player_lists();
        let best = manager.find_best_supply_center(query_object_id, world);
        self.sync_supply_lists_from_manager(&manager);
        best
    }

    fn resource_manager_from_player_lists(&self) -> ResourceGatheringManager {
        let mut manager = ResourceGatheringManager::new();
        for &warehouse_id in &self.supply_warehouses {
            manager.add_supply_warehouse(warehouse_id);
        }
        for &center_id in &self.supply_centers {
            manager.add_supply_center(center_id);
        }
        manager
    }

    fn sync_supply_lists_from_manager(&mut self, manager: &ResourceGatheringManager) {
        self.supply_warehouses = manager.get_supply_warehouses().iter().copied().collect();
        self.supply_centers = manager.get_supply_centers().iter().copied().collect();
    }

    // =========================================================
    // Squad System - Hotkey Squads (C++ Player.h line 382)
    // =========================================================

    /// Get a hotkey squad by number
    /// C++ Reference: Player::getHotkeySquad() (Player.h line 429)
    pub fn get_hotkey_squad(&mut self, squad_number: i32) -> Option<&mut Squad> {
        if squad_number >= 0 && (squad_number as usize) < NUM_HOTKEY_SQUADS {
            Some(&mut self.hotkey_squads[squad_number as usize])
        } else {
            None
        }
    }

    /// Get hotkey squad (const access)
    pub fn get_hotkey_squad_const(&self, squad_number: i32) -> Option<&Squad> {
        if squad_number >= 0 && (squad_number as usize) < NUM_HOTKEY_SQUADS {
            Some(&self.hotkey_squads[squad_number as usize])
        } else {
            None
        }
    }

    /// Get the squad number for an object, or NO_HOTKEY_SQUAD if not in any
    /// C++ Reference: Player::getSquadNumberForObject() (Player.cpp)
    pub fn get_squad_number_for_object(&self, object_id: ObjectID) -> i32 {
        for (i, squad) in self.hotkey_squads.iter().enumerate() {
            if squad.contains(object_id) {
                return i as i32;
            }
        }
        NO_HOTKEY_SQUAD
    }

    /// Remove an object from all hotkey squads
    /// C++ Reference: Player::removeObjectFromHotkeySquad() (Player.cpp)
    pub fn remove_object_from_hotkey_squad(&mut self, object_id: ObjectID) {
        for squad in &mut self.hotkey_squads {
            squad.remove_object(object_id);
        }
    }

    /// Clear a specific hotkey squad
    pub fn clear_hotkey_squad(&mut self, squad_number: i32) {
        if let Some(squad) = self.get_hotkey_squad(squad_number) {
            squad.clear();
        }
    }

    /// C++ `Player::processCreateTeamGameMessage` (`Player.cpp:3629-3648`).
    pub fn process_create_team_game_message(&mut self, hotkey_num: i32, object_ids: &[ObjectID]) {
        if hotkey_num < 0 || (hotkey_num as usize) >= NUM_HOTKEY_SQUADS {
            return;
        }
        self.hotkey_squads[hotkey_num as usize].clear_squad();
        for &object_id in object_ids {
            self.remove_object_from_hotkey_squad(object_id);
            self.hotkey_squads[hotkey_num as usize].add_object(object_id);
        }
    }

    /// C++ `Player::processSelectTeamGameMessage` (`Player.cpp:3654-3678`).
    pub fn process_select_team_game_message(&mut self, hotkey_num: i32) {
        if hotkey_num < 0 || (hotkey_num as usize) >= NUM_HOTKEY_SQUADS {
            return;
        }
        self.current_selection.clear_squad();
        for &object_id in self.hotkey_squads[hotkey_num as usize]
            .get_live_objects()
            .iter()
        {
            self.current_selection.add_object(object_id);
        }
    }

    /// C++ `Player::processAddTeamGameMessage` (`Player.cpp:3684-3703`).
    pub fn process_add_team_game_message(&mut self, hotkey_num: i32) {
        if hotkey_num < 0 || (hotkey_num as usize) >= NUM_HOTKEY_SQUADS {
            return;
        }
        for &object_id in self.hotkey_squads[hotkey_num as usize]
            .get_live_objects()
            .iter()
        {
            self.current_selection.add_object(object_id);
        }
    }

    // =========================================================
    // Current Selection Tracking (C++ Player.h line 383)
    // =========================================================

    /// Get the current selection squad
    /// C++ Reference: m_currentSelection usage throughout Player.cpp
    pub fn get_current_selection(&self) -> &Squad {
        &self.current_selection
    }

    /// Get mutable current selection
    pub fn get_current_selection_mut(&mut self) -> &mut Squad {
        &mut self.current_selection
    }

    /// Clear current selection
    pub fn clear_current_selection(&mut self) {
        self.current_selection.clear();
    }

    /// Add object to current selection
    pub fn add_to_current_selection(&mut self, object_id: ObjectID) {
        self.current_selection.add_object(object_id);
    }

    /// Remove object from current selection
    pub fn remove_from_current_selection(&mut self, object_id: ObjectID) {
        self.current_selection.remove_object(object_id);
    }

    /// Check if object is in current selection
    pub fn is_in_current_selection(&self, object_id: ObjectID) -> bool {
        self.current_selection.contains(object_id)
    }

    /// Get current selection size
    pub fn get_current_selection_size(&self) -> usize {
        self.current_selection.len()
    }
}
