// ============================================================================
// SUPPLY PILE SYSTEM
// ============================================================================

/// Supply pile with limited resources
/// Matches C++ supply warehouse mechanics with depletion support
#[derive(Debug, Clone)]
pub struct SupplyPile {
    /// Object ID of the pile
    pub pile_id: ObjectID,
    /// Position of the pile
    pub position: Coord3D,
    /// Remaining supply boxes
    pub remaining_boxes: i32,
    /// Maximum supply boxes (for visualization)
    pub max_boxes: i32,
    /// Number of active gatherers
    pub active_gatherers: usize,
    /// Whether pile is depleted
    pub is_depleted: bool,
}

impl SupplyPile {
    pub fn new(pile_id: ObjectID, position: Coord3D, initial_boxes: i32) -> Self {
        Self {
            pile_id,
            position,
            remaining_boxes: initial_boxes,
            max_boxes: initial_boxes,
            active_gatherers: 0,
            is_depleted: false,
        }
    }

    /// Take one box from the pile
    /// Returns true if successful, false if pile is empty
    pub fn take_box(&mut self) -> bool {
        if self.remaining_boxes > 0 {
            self.remaining_boxes -= 1;
            if self.remaining_boxes == 0 {
                self.is_depleted = true;
            }
            true
        } else {
            false
        }
    }

    /// Add a gatherer to this pile
    pub fn add_gatherer(&mut self) {
        self.active_gatherers += 1;
    }

    /// Remove a gatherer from this pile
    pub fn remove_gatherer(&mut self) {
        if self.active_gatherers > 0 {
            self.active_gatherers -= 1;
        }
    }

    /// Get the percentage of resources remaining (0.0 to 1.0)
    pub fn get_remaining_percentage(&self) -> f32 {
        if self.max_boxes > 0 {
            self.remaining_boxes as f32 / self.max_boxes as f32
        } else {
            0.0
        }
    }

    /// Check if pile can support another gatherer
    /// Typically limit to prevent overcrowding
    pub fn can_accept_gatherer(&self, max_gatherers_per_pile: usize) -> bool {
        !self.is_depleted && self.active_gatherers < max_gatherers_per_pile
    }
}

// ============================================================================
// FACTION-SPECIFIC SUPPLY MECHANICS
// ============================================================================

/// USA-specific supply drop zone
/// Implements Chinook supply drop mechanics
#[derive(Debug, Clone)]
pub struct SupplyDropZone {
    /// Zone ID
    pub zone_id: ObjectID,
    /// Drop zone position
    pub position: Coord3D,
    /// Supply value per drop
    pub supply_per_drop: u32,
    /// Cooldown between drops (in frames)
    pub drop_cooldown: u32,
    /// Next frame when drop is available
    pub next_drop_frame: u32,
    /// Whether zone is available
    pub is_available: bool,
}

impl SupplyDropZone {
    pub fn new(zone_id: ObjectID, position: Coord3D, supply_per_drop: u32, cooldown: u32) -> Self {
        Self {
            zone_id,
            position,
            supply_per_drop,
            drop_cooldown: cooldown,
            next_drop_frame: 0,
            is_available: true,
        }
    }

    /// Request a supply drop
    /// Returns the supply amount if available, 0 otherwise
    pub fn request_drop(&mut self, current_frame: u32) -> u32 {
        if self.is_available && current_frame >= self.next_drop_frame {
            self.next_drop_frame = current_frame + self.drop_cooldown;
            self.supply_per_drop
        } else {
            0
        }
    }

    /// Get cooldown remaining in frames
    pub fn get_cooldown_remaining(&self, current_frame: u32) -> u32 {
        if current_frame < self.next_drop_frame {
            self.next_drop_frame - current_frame
        } else {
            0
        }
    }
}

/// China-specific hacker income
/// Implements passive income from hacker units
#[derive(Debug, Clone)]
pub struct HackerIncome {
    /// Hacker object ID
    pub hacker_id: ObjectID,
    /// Income per interval
    pub income_per_interval: u32,
    /// Interval in frames
    pub income_interval: u32,
    /// Next frame to award income
    pub next_income_frame: u32,
    /// Whether hacker is active (not disabled/garrisoned)
    pub is_active: bool,
}

impl HackerIncome {
    pub fn new(
        hacker_id: ObjectID,
        income_per_interval: u32,
        interval: u32,
        current_frame: u32,
    ) -> Self {
        Self {
            hacker_id,
            income_per_interval,
            income_interval: interval,
            next_income_frame: current_frame + interval,
            is_active: true,
        }
    }

    /// Update and award income if ready
    /// Returns the income amount if awarded, 0 otherwise
    pub fn update(&mut self, current_frame: u32) -> u32 {
        if self.is_active && current_frame >= self.next_income_frame {
            self.next_income_frame = current_frame + self.income_interval;
            self.income_per_interval
        } else {
            0
        }
    }

    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }
}

/// GLA-specific black market income
/// Implements passive income from black market building
#[derive(Debug, Clone)]
pub struct BlackMarketIncome {
    /// Black market object ID
    pub market_id: ObjectID,
    /// Base income per interval
    pub base_income: u32,
    /// Interval in frames
    pub income_interval: u32,
    /// Next frame to award income
    pub next_income_frame: u32,
    /// Number of upgrade levels (increases income)
    pub upgrade_level: u32,
    /// Whether market is functional
    pub is_functional: bool,
}

impl BlackMarketIncome {
    pub fn new(market_id: ObjectID, base_income: u32, interval: u32, current_frame: u32) -> Self {
        Self {
            market_id,
            base_income,
            income_interval: interval,
            next_income_frame: current_frame + interval,
            upgrade_level: 0,
            is_functional: true,
        }
    }

    /// Update and award income if ready
    /// Returns the income amount if awarded, 0 otherwise
    pub fn update(&mut self, current_frame: u32) -> u32 {
        if self.is_functional && current_frame >= self.next_income_frame {
            self.next_income_frame = current_frame + self.income_interval;
            // Income increases with upgrade level
            self.base_income + (self.upgrade_level * 10)
        } else {
            0
        }
    }

    pub fn upgrade(&mut self) {
        self.upgrade_level += 1;
    }

    pub fn set_functional(&mut self, functional: bool) {
        self.is_functional = functional;
    }
}

