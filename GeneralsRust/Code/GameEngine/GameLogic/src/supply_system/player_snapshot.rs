// ============================================================================
// PLAYER SUPPLY MANAGEMENT
// ============================================================================

/// Player's supply management system
#[derive(Debug)]
pub struct PlayerSupplyManager {
    /// Player's money account
    money: Arc<RwLock<Money>>,
    /// Resource gathering manager
    resource_manager: Arc<RwLock<ResourceGatheringManager>>,
    /// Supply box value (can be modified by upgrades)
    supply_box_value: u32,
    /// Player's faction
    faction: Faction,
    /// Supply piles accessible to this player
    supply_piles: Vec<SupplyPile>,
    /// USA: Supply drop zones
    supply_drop_zones: Vec<SupplyDropZone>,
    /// China: Hacker income sources
    hacker_incomes: Vec<HackerIncome>,
    /// GLA: Black market income
    black_market: Option<BlackMarketIncome>,
}

impl PlayerSupplyManager {
    pub fn new(player_index: PlayerIndex, starting_money: u32) -> Self {
        Self::new_with_faction(player_index, starting_money, Faction::USA)
    }

    pub fn new_with_faction(
        player_index: PlayerIndex,
        starting_money: u32,
        faction: Faction,
    ) -> Self {
        Self {
            money: Arc::new(RwLock::new(Money::new(player_index, starting_money))),
            resource_manager: Arc::new(RwLock::new(ResourceGatheringManager::new())),
            supply_box_value: BASE_VALUE_PER_SUPPLY_BOX as u32,
            faction,
            supply_piles: Vec::new(),
            supply_drop_zones: Vec::new(),
            hacker_incomes: Vec::new(),
            black_market: None,
        }
    }

    pub fn get_money(&self) -> Arc<RwLock<Money>> {
        Arc::clone(&self.money)
    }

    pub fn get_resource_manager(&self) -> Arc<RwLock<ResourceGatheringManager>> {
        Arc::clone(&self.resource_manager)
    }

    pub fn get_supply_box_value(&self) -> u32 {
        self.supply_box_value
    }

    pub fn set_supply_box_value(&mut self, value: u32) {
        self.supply_box_value = value;
    }

    pub fn get_faction(&self) -> Faction {
        self.faction
    }

    // Supply pile management
    pub fn add_supply_pile(&mut self, pile: SupplyPile) {
        self.supply_piles.push(pile);
    }

    pub fn remove_supply_pile(&mut self, pile_id: ObjectID) {
        self.supply_piles.retain(|p| p.pile_id != pile_id);
    }

    pub fn get_supply_piles(&self) -> &[SupplyPile] {
        &self.supply_piles
    }

    pub fn get_supply_piles_mut(&mut self) -> &mut Vec<SupplyPile> {
        &mut self.supply_piles
    }

    // USA: Supply drop zone management
    pub fn add_supply_drop_zone(&mut self, zone: SupplyDropZone) {
        if self.faction == Faction::USA {
            self.supply_drop_zones.push(zone);
        }
    }

    pub fn request_supply_drop(&mut self, zone_id: ObjectID, current_frame: u32) -> u32 {
        if self.faction != Faction::USA {
            return 0;
        }

        for zone in &mut self.supply_drop_zones {
            if zone.zone_id == zone_id {
                return zone.request_drop(current_frame);
            }
        }
        0
    }

    // China: Hacker income management
    pub fn add_hacker(&mut self, hacker: HackerIncome) {
        if self.faction == Faction::China {
            self.hacker_incomes.push(hacker);
        }
    }

    pub fn remove_hacker(&mut self, hacker_id: ObjectID) {
        self.hacker_incomes.retain(|h| h.hacker_id != hacker_id);
    }

    pub fn update_hacker_income(&mut self, current_frame: u32) -> u32 {
        if self.faction != Faction::China {
            return 0;
        }

        let mut total_income = 0;
        for hacker in &mut self.hacker_incomes {
            total_income += hacker.update(current_frame);
        }
        total_income
    }

    // GLA: Black market management
    pub fn set_black_market(&mut self, market: BlackMarketIncome) {
        if self.faction == Faction::GLA {
            self.black_market = Some(market);
        }
    }

    pub fn update_black_market_income(&mut self, current_frame: u32) -> u32 {
        if self.faction != Faction::GLA {
            return 0;
        }

        if let Some(market) = &mut self.black_market {
            market.update(current_frame)
        } else {
            0
        }
    }

    pub fn upgrade_black_market(&mut self) {
        if let Some(market) = &mut self.black_market {
            market.upgrade();
        }
    }
}

// C++ parity: SupplyTruckAIUpdate saves its state machine, preferred dock,
// carried box count, and force-wanting latch. The force-busy latch is transient
// in the original class and is intentionally not serialized.
impl Snapshotable for SupplyTruckAIUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());

        let mut version: u8 = 1;
        xfer_io(xfer.xfer_version(&mut version, 1))?;

        if let Some(state_machine) = &mut self.state_machine {
            state_machine
                .machine
                .lock()
                .map_err(|_| "SupplyTruckStateMachine lock poisoned".to_string())?
                .xfer(xfer)
                .map_err(|e| e.to_string())?;
            self.sync_state_from_machine();
        } else {
            let mut state_disc = supply_truck_state_to_id(self.state);
            xfer_io(xfer.xfer_unsigned_int(&mut state_disc))?;
            if xfer.get_xfer_mode() == game_engine::common::system::XferMode::Load {
                self.state = supply_truck_state_from_id(state_disc);
            }
        }

        let mut preferred_dock = self.preferred_dock.unwrap_or(INVALID_ID);
        xfer_io(xfer.xfer_unsigned_int(&mut preferred_dock))?;
        if xfer.get_xfer_mode() == game_engine::common::system::XferMode::Load {
            self.preferred_dock = (preferred_dock != INVALID_ID).then_some(preferred_dock);
        }

        xfer_io(xfer.xfer_int(&mut self.number_boxes))?;
        xfer_io(xfer.xfer_bool(&mut self.force_wanting_state))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn supply_truck_state_to_id(state: SupplyTruckState) -> u32 {
    match state {
        SupplyTruckState::Idle => ST_IDLE,
        SupplyTruckState::Busy => ST_BUSY,
        SupplyTruckState::Wanting => ST_WANTING,
        SupplyTruckState::Regrouping => ST_REGROUPING,
        SupplyTruckState::Docking => ST_DOCKING,
    }
}

fn supply_truck_state_from_id(state_id: u32) -> SupplyTruckState {
    match state_id {
        ST_BUSY => SupplyTruckState::Busy,
        ST_WANTING => SupplyTruckState::Wanting,
        ST_REGROUPING => SupplyTruckState::Regrouping,
        ST_DOCKING => SupplyTruckState::Docking,
        _ => SupplyTruckState::Idle,
    }
}

// C++ parity: WorkerAIUpdateModuleData and WorkerAIUpdate save/load
impl Snapshotable for WorkerAIUpdateData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());
        xfer_io(xfer.xfer_int(&mut self.max_boxes))?;
        xfer_io(xfer.xfer_real(&mut self.warehouse_scan_distance))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.warehouse_delay))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.center_delay))?;
        xfer_io(xfer.xfer_real(&mut self.repair_health_percent_per_second))?;
        xfer_io(xfer.xfer_real(&mut self.bored_time))?;
        xfer_io(xfer.xfer_real(&mut self.bored_range))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.upgraded_supply_boost))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for WorkerAIUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());

        self.data.xfer(xfer)?;

        let mut state_disc = self.state as u32;
        xfer_io(xfer.xfer_unsigned_int(&mut state_disc))?;
        self.state = match state_disc {
            0 => SupplyTruckState::Idle,
            1 => SupplyTruckState::Busy,
            2 => SupplyTruckState::Wanting,
            3 => SupplyTruckState::Regrouping,
            _ => SupplyTruckState::Idle,
        };

        xfer_io(xfer.xfer_int(&mut self.number_boxes))?;

        let mut has_dock = self.preferred_dock.is_some() as u32;
        xfer_io(xfer.xfer_unsigned_int(&mut has_dock))?;
        if has_dock != 0 {
            let mut dock_id = self.preferred_dock.unwrap_or(0);
            xfer_io(xfer.xfer_unsigned_int(&mut dock_id))?;
            self.preferred_dock = Some(dock_id);
        } else {
            self.preferred_dock = None;
        }

        xfer_io(xfer.xfer_bool(&mut self.force_wanting_state))?;
        xfer_io(xfer.xfer_bool(&mut self.force_busy_state))?;

        let mut dozer_action_disc = self.dozer_action_state as u32;
        xfer_io(xfer.xfer_unsigned_int(&mut dozer_action_disc))?;
        self.dozer_action_state = match dozer_action_disc {
            0 => WorkerDozerActionState::PickActionPos,
            1 => WorkerDozerActionState::MoveToActionPos,
            2 => WorkerDozerActionState::DoAction,
            _ => WorkerDozerActionState::PickActionPos,
        };

        let mut has_task = self.current_task.is_some() as u32;
        xfer_io(xfer.xfer_unsigned_int(&mut has_task))?;
        if has_task != 0 {
            let mut task_disc = self.current_task.map(|t| t as u32).unwrap_or(0);
            xfer_io(xfer.xfer_unsigned_int(&mut task_disc))?;
            self.current_task = match task_disc {
                0 => Some(WorkerDozerTaskSlot::Build),
                1 => Some(WorkerDozerTaskSlot::Repair),
                2 => Some(WorkerDozerTaskSlot::Fortify),
                _ => None,
            };
        } else {
            self.current_task = None;
        }

        xfer_io(xfer.xfer_unsigned_int(&mut self.object_id))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.player_index))?;

        for entry in &mut self.dozer_tasks {
            xfer_io(xfer.xfer_unsigned_int(&mut entry.target_id))?;
            xfer_io(xfer.xfer_unsigned_int(&mut entry.order_frame))?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

