use super::*;
use serde::{Deserialize, Serialize};

/// C++ ProductionUpdateModuleData default MaxQueueEntries.
pub const DEFAULT_PRODUCTION_QUEUE_LIMIT: usize = 9;

/// Generals simulation logic runs at 30 updates per second.  C++ stores the
/// QueueProductionExitUpdate countdown as an integer number of those updates,
/// not as elapsed seconds.
const PRODUCTION_EXIT_LOGIC_FRAMES_PER_SECOND: f32 = 30.0;

/// Building-specific data and behaviors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildingData {
    pub building_type: BuildingType,
    pub production_queue: Vec<ProductionItem>,
    /// Wave 985: C++ ProductionUpdate pause residual (hold queue timer).
    pub production_paused: bool,
    /// C++ QueueProductionExitUpdate exit countdown residual (seconds).
    /// Compatibility/presentation mirror of the integer frame counter below.
    /// New source-backed Queue exits never use this float as authority.
    pub exit_delay_remaining: f32,
    /// C++ `QueueProductionExitUpdate::m_currentDelay`, in logic frames.
    /// `serde(default)` keeps float-only legacy snapshots loadable; the first
    /// parsed Queue tick reconstructs a conservative frame count from the
    /// compatibility value.
    #[serde(default)]
    pub exit_delay_remaining_frames: u32,
    /// C++ `QueueProductionExitUpdate::m_currentBurstCount`.
    #[serde(default)]
    pub exit_burst_remaining: u32,
    /// Whether the two Queue counters were initialized from an authored
    /// `QueueProductionExitUpdate` for this producer Object.
    #[serde(default)]
    pub queue_exit_state_initialized: bool,
    pub rally_point: Option<Vec3>,
    pub power_output: i32,
    pub power_requirement: i32,
    pub garrisoned_units: Vec<ObjectId>,
    pub max_garrison: usize,
    /// C++ parity (OpenContainModuleData::m_damagePercentageToUnits): percentage
    /// of damage passed to contained units when this building is destroyed.
    /// 0.0 = no damage (default), 1.0 = full max-health damage.
    pub damage_percent_to_units: f32,
    /// C++ GarrisonContain::m_originalTeam — restored when the last occupant leaves.
    #[serde(default)]
    pub original_team: Option<Team>,
    /// C++ GarrisonContain::m_hideGarrisonedStateFromNonallies.
    #[serde(default)]
    pub hide_garrisoned_state: bool,
    /// C++ GarrisonPointData effect drawables (`GarrisonGun` + FIRING_A).
    #[serde(default)]
    pub garrison_guns: Vec<GarrisonGunEffect>,
    /// C++ GarrisonContain::m_evacDisposition (1 left / 2 right / 3 burst).
    #[serde(default)]
    pub evac_disposition: u8,
    /// Cached world FIREPOINT bones (C++ m_garrisonPoint[condition]).
    #[serde(default)]
    pub garrison_fire_points: Vec<glam::Vec3>,
    /// Cached world STATION bones for non-enclosing Fire Base.
    #[serde(default)]
    pub garrison_station_points: Vec<glam::Vec3>,
    /// Occupant assigned to each FIREPOINT / STATION index.
    #[serde(default)]
    pub garrison_point_occupant: Vec<Option<ObjectId>>,
    /// C++ GarrisonContain::m_garrisonPointsInitialized.
    #[serde(default)]
    pub garrison_points_initialized: bool,
    /// C++ m_garrisonPoint[GARRISON_POINT_DAMAGED].
    #[serde(default)]
    pub garrison_fire_points_damaged: Vec<glam::Vec3>,
    /// C++ m_garrisonPoint[GARRISON_POINT_REALLY_DAMAGED].
    #[serde(default)]
    pub garrison_fire_points_really_damaged: Vec<glam::Vec3>,
    /// Last `findConditionIndex` applied to occupant FIREPOINT picks.
    #[serde(default)]
    pub garrison_points_condition: u8,
    /// C++ OpenContain::m_whichExitPath (1-based ExitStart/End cycle).
    #[serde(default)]
    pub which_exit_path: u8,
}

/// Live residual of C++ `GarrisonGun` fire-point drawables.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GarrisonGunEffect {
    #[serde(default)]
    pub drawable_id: Option<ObjectId>,
    #[serde(default)]
    pub last_effect_frame: u32,
    #[serde(default)]
    pub firing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildingType {
    CommandCenter,
    Barracks,
    WarFactory,
    Airfield,
    RepairPad,
    HealPad,
    SupplyCenter,
    PowerPlant,
    DefenseTurret,
    SupplyDropZone,
    Palace,
    Propaganda,
    Bunker,
}

impl BuildingType {
    /// Derive building type from a template name using simple heuristics.
    pub fn from_template_name(name: &str) -> Self {
        let lower = name.to_ascii_lowercase();
        if lower.contains("barracks") {
            BuildingType::Barracks
        } else if lower.contains("warfactory")
            || lower.contains("war factory")
            // GLA vehicle factory — must not fall through to CommandCenter.
            || lower.contains("armsdealer")
            || lower.contains("arms_dealer")
            || lower.contains("arms dealer")
        {
            BuildingType::WarFactory
        } else if lower.contains("airfield") || lower.contains("air field") {
            BuildingType::Airfield
        } else if lower.contains("repair") {
            BuildingType::RepairPad
        } else if lower.contains("hospital") || lower.contains("heal") || lower.contains("medic") {
            BuildingType::HealPad
        } else if lower.contains("dropzone")
            || lower.contains("drop zone")
            || lower.contains("supplydropzone")
        {
            // Before generic "supply" so AmericaSupplyDropZone is not SupplyCenter.
            BuildingType::SupplyDropZone
        } else if lower.contains("supply") || lower.contains("stash") {
            BuildingType::SupplyCenter
        } else if lower.contains("power") {
            BuildingType::PowerPlant
        } else if lower.contains("patri") || lower.contains("turret") || lower.contains("defense") {
            BuildingType::DefenseTurret
        } else if lower.contains("palace") {
            BuildingType::Palace
        } else if lower.contains("propaganda") {
            BuildingType::Propaganda
        } else if lower.contains("bunker") {
            BuildingType::Bunker
        } else if lower.contains("command") || lower.contains("center") {
            BuildingType::CommandCenter
        } else {
            BuildingType::CommandCenter
        }
    }
}

/// C++ ProductionType residual (ProductionUpdate.h).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProductionKind {
    /// PRODUCTION_UNIT residual.
    Unit,
    /// PRODUCTION_UPGRADE residual.
    Upgrade,
}

/// A completed production queue head.  Unit entries may release their whole
/// remaining QuantityModifier batch in one C++ `ProductionUpdate::update`;
/// upgrade entries remain a single completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionCompletion {
    pub template_name: String,
    pub kind: ProductionKind,
    pub quantity: u32,
}

/// Mutable C++ `QueueProductionExitUpdate` state mirrored across the
/// host/GameWorld boundary.  This stays separate from immutable Object INI
/// metadata, because every producer instance has its own delay/burst counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductionExitRuntimeState {
    pub delay_frames: u32,
    pub burst_remaining: u32,
    pub initialized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionItem {
    pub template_name: String,
    /// Compatibility/presentation progress in seconds.  C++ completion is
    /// driven by `frames_under_construction`, never by this float.
    pub progress: f32,
    pub total_time: f32,
    /// C++ `ProductionEntry::m_framesUnderConstruction` (incremented once per
    /// logic update).  `serde(default)` migrates old float-only save snapshots
    /// through [`Self::migrate_legacy_construction_frames`] on their next tick.
    #[serde(default)]
    pub construction_frames: u32,
    pub cost: Resources,
    /// C++ ProductionEntry::m_productionQuantityTotal residual.
    pub quantity_total: u32,
    /// C++ ProductionEntry::m_productionQuantityProduced residual.
    pub quantity_produced: u32,
    /// C++ ProductionEntry::m_type residual.
    pub kind: ProductionKind,
}

impl ProductionItem {
    pub fn is_upgrade(&self) -> bool {
        matches!(self.kind, ProductionKind::Upgrade)
    }

    /// C++ `ProductionEntry::getProductionQuantityRemaining`.
    pub fn remaining_quantity(&self) -> u32 {
        self.quantity_total
            .max(1)
            .saturating_sub(self.quantity_produced)
    }

    fn total_construction_frames(&self, power_factor: f32) -> u32 {
        gamelogic::world::entities::production_total_logic_frames(
            self.total_time,
            self.is_upgrade(),
            power_factor,
        )
    }

    fn migrate_legacy_construction_frames(&mut self, power_factor: f32) {
        if self.construction_frames == 0 && self.progress > 0.0 {
            self.construction_frames =
                gamelogic::world::entities::production_frames_from_legacy_progress(
                    self.progress,
                    self.total_time,
                    self.total_construction_frames(power_factor),
                );
        }
    }

    fn advance_one_construction_frame(&mut self, power_factor: f32) {
        self.migrate_legacy_construction_frames(power_factor);
        let total_frames = self.total_construction_frames(power_factor);
        if total_frames == 0 {
            self.progress = self.total_time.max(0.0);
            return;
        }
        self.construction_frames = self.construction_frames.saturating_add(1);
        self.progress = gamelogic::world::entities::production_progress_from_logic_frames(
            self.construction_frames,
            total_frames,
            self.total_time,
        );
    }

    fn is_complete_at_power(&mut self, power_factor: f32) -> bool {
        self.migrate_legacy_construction_frames(power_factor);
        let total_frames = self.total_construction_frames(power_factor);
        total_frames == 0 || self.construction_frames >= total_frames
    }
}

impl BuildingData {
    pub fn new(building_type: BuildingType) -> Self {
        // Fail-closed residual: only bunker-style garrisonable structures accept
        // infantry. Faction producers (barracks/factories/CC) are not garrison
        // containers in retail — capacity stays 0 so Enter rejects them.
        let (power_output, power_requirement, max_garrison) = match building_type {
            BuildingType::CommandCenter => (0, -3, 0),
            BuildingType::Barracks => (0, -1, 0),
            BuildingType::WarFactory => (0, -2, 0),
            BuildingType::Airfield => (0, -3, 0),
            BuildingType::RepairPad => (0, -1, 0),
            BuildingType::HealPad => (0, -1, 0),
            BuildingType::SupplyCenter => (0, -1, 0),
            BuildingType::PowerPlant => (10, 0, 0),
            BuildingType::DefenseTurret => (0, -2, 0),
            BuildingType::SupplyDropZone => (0, 0, 0),
            BuildingType::Palace => (0, -5, 0),
            BuildingType::Propaganda => (0, -1, 0),
            // Civilian bunkers / garrison buildings — residual capacity (not full
            // C++ GarrisonContain max or fire-point matrix).
            BuildingType::Bunker => (0, 0, 5),
        };

        Self {
            building_type,
            production_queue: Vec::new(),
            production_paused: false,
            exit_delay_remaining: 0.0,
            exit_delay_remaining_frames: 0,
            exit_burst_remaining: 0,
            queue_exit_state_initialized: false,
            rally_point: None,
            power_output,
            power_requirement,
            garrisoned_units: Vec::new(),
            max_garrison,
            damage_percent_to_units: 0.0,
            original_team: None,
            hide_garrisoned_state: false,
            garrison_guns: Vec::new(),
            evac_disposition: 3,
            garrison_fire_points: Vec::new(),
            garrison_station_points: Vec::new(),
            garrison_point_occupant: Vec::new(),
            garrison_points_initialized: false,
            garrison_fire_points_damaged: Vec::new(),
            garrison_fire_points_really_damaged: Vec::new(),
            garrison_points_condition: 0,
            which_exit_path: 0,
        }
    }

    pub fn can_produce(&self, template: &ThingTemplate) -> bool {
        match self.building_type {
            // C++ isPossibleToMakeUnit: command-set UNIT_BUILD of that
            // ThingTemplate. Barracks command sets only list infantry
            // (KINDOF_INFANTRY). Name-contains is not a C++ gate.
            BuildingType::Barracks => template.is_kind_of(KindOf::Infantry),
            BuildingType::WarFactory => template.is_kind_of(KindOf::Vehicle),
            BuildingType::Airfield => template.is_kind_of(KindOf::Aircraft),
            // Retail SupplyCenterProductionExitUpdate builds only the faction's
            // HARVESTER listed in its CommandSet (Chinook, Supply Truck, or
            // Worker).  Exact CommandSet authorization is checked before this
            // broad producer capability; this is the typed queue gate.
            BuildingType::SupplyCenter => template.is_kind_of(KindOf::Harvester),
            BuildingType::CommandCenter => {
                // Command centers can produce workers/dozers
                template.name.contains("Worker") || template.name.contains("Dozer")
            }
            _ => false,
        }
    }

    pub fn add_to_queue(&mut self, template_name: String, template: &ThingTemplate) -> bool {
        self.add_to_queue_with_quantity(template_name, template, 1)
    }

    /// Enqueue with ProductionUpdate QuantityModifier residual count.
    pub fn add_to_queue_with_quantity(
        &mut self,
        template_name: String,
        template: &ThingTemplate,
        quantity: u32,
    ) -> bool {
        self.add_to_queue_with_quantity_and_terms(
            template_name,
            template,
            quantity,
            template.build_time,
            template.build_cost,
        )
    }

    /// Enqueue an authored unit with the player-specific terms already
    /// calculated by `ThingTemplate::calcCostToBuild` /
    /// `ThingTemplate::calcTimeToBuild`.
    ///
    /// `ProductionEntry` stores the charged cost and its owning player's
    /// duration.  Keeping those values on the queue item makes a later cancel
    /// refund the exact amount paid and prevents another player's General
    /// modifier from changing an in-flight entry.
    pub fn add_to_queue_with_quantity_and_terms(
        &mut self,
        template_name: String,
        template: &ThingTemplate,
        quantity: u32,
        total_time: f32,
        cost: Resources,
    ) -> bool {
        if self.can_produce(template)
            && self.production_queue.len() < DEFAULT_PRODUCTION_QUEUE_LIMIT
        {
            let item = ProductionItem {
                template_name,
                progress: 0.0,
                total_time: total_time.max(0.0),
                construction_frames: 0,
                cost,
                quantity_total: quantity.max(1),
                quantity_produced: 0,
                kind: ProductionKind::Unit,
            };
            self.production_queue.push(item);
            true
        } else {
            false
        }
    }

    /// Queue a PRODUCTION_UPGRADE residual entry (research on this producer).
    ///
    /// C++ ProductionUpdate::queueUpgrade — one entry per upgrade name, costs
    /// already withdrawn by the player path. `total_time` is research seconds.
    pub fn add_upgrade_to_queue(
        &mut self,
        upgrade_name: String,
        total_time_secs: f32,
        cost: Resources,
    ) -> bool {
        if self.production_queue.len() >= DEFAULT_PRODUCTION_QUEUE_LIMIT {
            return false;
        }
        // C++ isUpgradeInQueue: refuse duplicate upgrade entries.
        let key = upgrade_name.to_ascii_lowercase();
        if self
            .production_queue
            .iter()
            .any(|i| i.is_upgrade() && i.template_name.eq_ignore_ascii_case(&upgrade_name))
        {
            return false;
        }
        let _ = key;
        self.production_queue.push(ProductionItem {
            template_name: upgrade_name,
            progress: 0.0,
            total_time: total_time_secs.max(0.0),
            construction_frames: 0,
            cost,
            quantity_total: 1,
            quantity_produced: 0,
            kind: ProductionKind::Upgrade,
        });
        true
    }

    /// C++ parity (ThingTemplate::calcTimeToBuild): when energy ratio < 1.0
    /// production speed is reduced.  The penalty is:
    ///   energy_short = (1.0 - ratio) * penalty_modifier
    ///   rate = max(1.0 - energy_short, MIN_SPEED)
    ///   if ratio < 1.0: rate = min(rate, MAX_SPEED)
    /// Defaults: MIN=0.5, MAX=0.8, modifier=1.0  (GameData.ini).
    pub fn update_production(
        &mut self,
        dt: f32,
        power_factor: f32,
    ) -> Option<ProductionCompletion> {
        self.tick_exit_delay(dt);
        self.advance_production_progress(dt, power_factor);
        self.try_complete_production_at_power(power_factor)
    }

    /// C++ QueueProductionExitUpdate door/exit residual.
    pub fn tick_exit_delay(&mut self, dt: f32) {
        if self.exit_delay_remaining > 0.0 {
            self.exit_delay_remaining = (self.exit_delay_remaining - dt).max(0.0);
        }
    }

    fn sync_exit_delay_presentation(&mut self) {
        self.exit_delay_remaining =
            self.exit_delay_remaining_frames as f32 / PRODUCTION_EXIT_LOGIC_FRAMES_PER_SECOND;
    }

    /// Lazily migrate a float-only legacy snapshot into C++ Queue runtime
    /// state.  A legacy snapshot cannot recover an already-partially-consumed
    /// InitialBurst, so an active legacy delay deliberately fails closed rather
    /// than manufacturing an extra immediate exit.
    fn initialize_queue_exit_state(&mut self, metadata: &ProductionExitMetadata) {
        debug_assert!(metadata.is_queue());
        if self.queue_exit_state_initialized {
            return;
        }
        if self.exit_delay_remaining_frames == 0 && self.exit_delay_remaining > 0.0 {
            self.exit_delay_remaining_frames = (self.exit_delay_remaining
                * PRODUCTION_EXIT_LOGIC_FRAMES_PER_SECOND)
                .ceil()
                .clamp(0.0, u32::MAX as f32) as u32;
        }
        if self.exit_delay_remaining_frames == 0 {
            self.exit_burst_remaining = metadata.initial_burst;
        } else {
            self.exit_burst_remaining = 0;
        }
        self.queue_exit_state_initialized = true;
        self.sync_exit_delay_presentation();
    }

    /// Advance C++ `QueueProductionExitUpdate::update` once.  Its burst state
    /// keeps an exit free and clears the current delay; otherwise an active
    /// delay counts down exactly one logic frame.  DefaultProductionExitUpdate
    /// has no busy state at all.
    pub fn tick_production_exit(&mut self, metadata: Option<&ProductionExitMetadata>, dt: f32) {
        match metadata {
            Some(metadata) if metadata.is_queue() => {
                self.initialize_queue_exit_state(metadata);
                if self.exit_burst_remaining > 0 || self.exit_delay_remaining_frames == 0 {
                    self.exit_delay_remaining_frames = 0;
                } else {
                    self.exit_delay_remaining_frames =
                        self.exit_delay_remaining_frames.saturating_sub(1);
                }
                self.sync_exit_delay_presentation();
            }
            Some(_) => {
                // C++ DefaultProductionExitUpdate owns no delay/burst fields.
                self.exit_delay_remaining = 0.0;
                self.exit_delay_remaining_frames = 0;
                self.exit_burst_remaining = 0;
                self.queue_exit_state_initialized = false;
            }
            None => self.tick_exit_delay(dt),
        }
    }

    /// Snapshot/Xfer-safe C++ QueueProductionExitUpdate state for GameWorld
    /// ownership.  The float remains a presentation/legacy compatibility
    /// mirror and is not used to decide a parsed Queue exit.
    pub fn production_exit_runtime_state(&self) -> ProductionExitRuntimeState {
        ProductionExitRuntimeState {
            delay_frames: self.exit_delay_remaining_frames,
            burst_remaining: self.exit_burst_remaining,
            initialized: self.queue_exit_state_initialized,
        }
    }

    pub fn set_production_exit_runtime_state(&mut self, state: ProductionExitRuntimeState) {
        self.exit_delay_remaining_frames = state.delay_frames;
        self.exit_burst_remaining = state.burst_remaining;
        self.queue_exit_state_initialized = state.initialized;
        if state.initialized {
            self.sync_exit_delay_presentation();
        }
    }

    /// C++ `QueueProductionExitUpdate::isFreeToExit` authority.  This
    /// initializes a parsed Queue instance before asking, so fresh state has
    /// its authored InitialBurst instead of a name-derived policy.
    pub fn production_head_exit_available(
        &mut self,
        metadata: Option<&ProductionExitMetadata>,
    ) -> bool {
        if self
            .production_queue
            .first()
            .is_some_and(ProductionItem::is_upgrade)
        {
            return true;
        }
        match metadata {
            Some(metadata) if metadata.is_queue() => {
                self.initialize_queue_exit_state(metadata);
                self.exit_burst_remaining > 0 || self.exit_delay_remaining_frames == 0
            }
            Some(_) => true,
            // Unparsed legacy producers retain their former residual, but no
            // source-authored Queue behavior is inferred from their name.
            None => self.exit_delay_remaining <= 0.0,
        }
    }

    /// Count how many members C++ ProductionUpdate may release this update.
    /// Queue exits reserve/arm state per member; Default exits have no busy
    /// state and therefore accept the complete remaining QuantityModifier
    /// batch in one update.
    pub fn production_exit_release_limit(
        &mut self,
        metadata: Option<&ProductionExitMetadata>,
        remaining: u32,
    ) -> u32 {
        if remaining == 0 {
            return 0;
        }
        let Some(metadata) = metadata else {
            return self
                .production_head_exit_available(None)
                .then_some(remaining)
                .unwrap_or(0);
        };
        if !metadata.is_queue() {
            return remaining;
        }
        self.initialize_queue_exit_state(metadata);
        let mut delay_frames = self.exit_delay_remaining_frames;
        let mut burst_remaining = self.exit_burst_remaining;
        let mut released = 0u32;
        while released < remaining && (burst_remaining > 0 || delay_frames == 0) {
            released = released.saturating_add(1);
            delay_frames = metadata.exit_delay_frames;
            if burst_remaining > 0 {
                burst_remaining = burst_remaining.saturating_sub(1);
            }
        }
        released
    }

    /// Mirror one successful C++ `QueueProductionExitUpdate::exitObjectViaDoor`.
    /// Call once for every member actually spawned/bound, after production has
    /// reserved that member's exit.
    pub fn record_successful_production_exit(&mut self, metadata: Option<&ProductionExitMetadata>) {
        match metadata {
            Some(metadata) if metadata.is_queue() => {
                self.initialize_queue_exit_state(metadata);
                self.exit_delay_remaining_frames = metadata.exit_delay_frames;
                if self.exit_burst_remaining > 0 {
                    self.exit_burst_remaining = self.exit_burst_remaining.saturating_sub(1);
                }
                self.sync_exit_delay_presentation();
            }
            Some(_) => {
                self.exit_delay_remaining = 0.0;
                self.exit_delay_remaining_frames = 0;
                self.exit_burst_remaining = 0;
                self.queue_exit_state_initialized = false;
            }
            None => {}
        }
    }

    /// Advance the head-of-queue exactly one C++ `ProductionUpdate::update`
    /// frame (no completion/spawn).  `dt` remains in this API for detached
    /// callers, but production itself is logic-frame based, not a float timer.
    /// Under GameWorld production authority, shadow owns this advance.
    pub fn advance_production_progress(&mut self, dt: f32, power_factor: f32) {
        // Wave 985: paused queue holds build timer (C++ ProductionUpdate pause).
        if self.production_paused {
            let _ = (dt, power_factor);
            return;
        }
        let _ = dt;
        if let Some(item) = self.production_queue.first_mut() {
            // Only advance build timer until the batch is fully produced residual.
            if item.quantity_produced == 0 {
                item.advance_one_construction_frame(power_factor);
            }
        }
    }

    /// Complete/release a full-power head with C++ QuantityModifier semantics.
    pub fn try_complete_production(&mut self) -> Option<ProductionCompletion> {
        self.try_complete_production_at_power(1.0)
    }

    /// Complete/release head-of-queue when C++ integer frame progress is done
    /// and exit delay is clear.  `ThingTemplate::calcTimeToBuild` recalculates
    /// its current power-adjusted integer threshold at this point.
    pub fn try_complete_production_at_power(
        &mut self,
        power_factor: f32,
    ) -> Option<ProductionCompletion> {
        self.try_complete_production_at_power_with_exit_metadata(power_factor, None, None)
    }

    /// Complete/release the queue head with C++ QuantityModifier semantics.
    ///
    /// A unit entry releases every remaining batch member once its exit is
    /// available.  `max_unit_releases` is only used by entity-first sole-tick
    /// writeback: it caps release to the number of authoritative ready events
    /// rather than inventing a missing completion.  It never changes upgrade
    /// completion behavior.
    pub fn try_complete_production_at_power_with_limit(
        &mut self,
        power_factor: f32,
        max_unit_releases: Option<u32>,
    ) -> Option<ProductionCompletion> {
        self.try_complete_production_at_power_with_exit_metadata(
            power_factor,
            None,
            max_unit_releases,
        )
    }

    /// Complete/release the queue head with immutable authored exit metadata.
    /// A caller only records the Queue exit state after each actual spawned
    /// member; this method merely mirrors C++ ProductionUpdate's per-member
    /// `reserveDoorForExit` availability loop.
    pub fn try_complete_production_at_power_with_exit_metadata(
        &mut self,
        power_factor: f32,
        exit_metadata: Option<&ProductionExitMetadata>,
        max_unit_releases: Option<u32>,
    ) -> Option<ProductionCompletion> {
        // A legacy/corrupt entry that already emitted its whole batch must not
        // wedge the queue.  Normal completion removes it in the same update.
        if self
            .production_queue
            .first()
            .is_some_and(|item| item.remaining_quantity() == 0)
        {
            self.production_queue.remove(0);
            return None;
        }
        let (is_complete, is_upgrade, remaining) = {
            let item = self.production_queue.first_mut()?;
            (
                item.is_complete_at_power(power_factor),
                item.is_upgrade(),
                item.remaining_quantity(),
            )
        };
        if !is_complete {
            return None;
        }
        let quantity = if is_upgrade {
            1
        } else {
            self.production_exit_release_limit(exit_metadata, remaining)
                .min(max_unit_releases.unwrap_or(u32::MAX))
        };
        if quantity == 0 {
            // Keep UI progress saturated while preserving the integer C++ frame
            // counter for a saved, door-blocked production entry.
            if let Some(item) = self.production_queue.first_mut() {
                item.progress = item.total_time.max(0.0);
            }
            return None;
        }
        let item = self.production_queue.first_mut()?;
        item.progress = item.total_time.max(0.0);
        item.quantity_produced = item.quantity_produced.saturating_add(quantity);
        let name = item.template_name.clone();
        let kind = item.kind;
        let done = item.quantity_produced >= item.quantity_total.max(1);
        if done {
            self.production_queue.remove(0);
        }
        Some(ProductionCompletion {
            template_name: name,
            kind,
            quantity,
        })
    }

    /// Query the C++ integer production threshold for the queue head.  This is
    /// used before factory-door handling and by GameWorld sole-tick writeback;
    /// it intentionally does not consult float UI progress.
    pub fn production_head_complete_at_power(&mut self, power_factor: f32) -> bool {
        self.production_queue
            .first_mut()
            .is_some_and(|item| item.is_complete_at_power(power_factor))
    }

    /// Arm QueueProductionExitUpdate residual after a unit exits.
    pub fn arm_exit_delay(&mut self, delay_seconds: f32) {
        self.exit_delay_remaining = delay_seconds.max(0.0);
        self.exit_delay_remaining_frames = (self.exit_delay_remaining
            * PRODUCTION_EXIT_LOGIC_FRAMES_PER_SECOND)
            .ceil()
            .clamp(0.0, u32::MAX as f32) as u32;
        self.exit_burst_remaining = 0;
        self.queue_exit_state_initialized = true;
    }

    pub fn exit_delay_remaining(&self) -> f32 {
        self.exit_delay_remaining
    }

    pub fn cancel_production(&mut self, index: usize) -> Option<ProductionItem> {
        if index < self.production_queue.len() {
            Some(self.production_queue.remove(index))
        } else {
            None
        }
    }

    /// Wave 985: pause/resume production queue residual.
    pub fn set_production_paused(&mut self, paused: bool) {
        self.production_paused = paused;
    }

    pub fn is_production_paused(&self) -> bool {
        self.production_paused
    }

    pub fn get_production_progress(&self) -> Option<f32> {
        self.production_queue.first().map(|item| {
            if item.total_time <= 0.0 {
                1.0
            } else {
                (item.progress / item.total_time).clamp(0.0, 1.0)
            }
        })
    }

    pub fn can_garrison(&self) -> bool {
        self.garrisoned_units.len() < self.max_garrison
    }

    pub fn garrison_unit(&mut self, unit_id: ObjectId) -> bool {
        if self.can_garrison() {
            self.garrisoned_units.push(unit_id);
            true
        } else {
            false
        }
    }

    pub fn ungarrison_unit(&mut self) -> Option<ObjectId> {
        self.garrisoned_units.pop()
    }

    /// C++ `GarrisonContain::removeObjectFromGarrisonPoint` /
    /// `removeObjectFromStationPoint`. FIREPOINT and STATION share this
    /// occupancy vector — clear by occupant id so a survivor can take the
    /// window. Do not wipe the whole list (other occupants keep their slots).
    pub fn free_garrison_point_for(&mut self, occupant_id: ObjectId) {
        for slot in &mut self.garrison_point_occupant {
            if *slot == Some(occupant_id) {
                *slot = None;
            }
        }
    }
}

/// Building factory functions
pub fn create_building_templates() -> HashMap<String, ThingTemplate> {
    let mut templates = HashMap::new();

    // GLA Buildings
    let mut gla_command = ThingTemplate::new("GLA_Command");
    gla_command
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(2500.0)
        .set_cost(2000, -3)
        .set_model("ubcmdhq"); // FactionBuilding.ini pristine GLA command center
    templates.insert("GLA_Command".to_string(), gla_command);

    let mut gla_barracks = ThingTemplate::new("GLA_Barracks");
    gla_barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0)
        .set_cost(500, -1)
        .set_model("ubbarracks"); // FactionBuilding.ini pristine GLA barracks
    templates.insert("GLA_Barracks".to_string(), gla_barracks);

    let mut gla_supply = ThingTemplate::new("GLA_SupplyStash");
    gla_supply
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(600.0)
        .set_cost(800, -1)
        .set_model("ubsupply"); // FactionBuilding.ini pristine GLA supply stash
    templates.insert("GLA_SupplyStash".to_string(), gla_supply);

    let mut gla_arms_dealer = ThingTemplate::new("GLA_ArmsDealer");
    gla_arms_dealer
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1200.0)
        .set_cost(800, -2)
        .set_model("ubarmdeal"); // FactionBuilding.ini pristine GLA arms dealer
    templates.insert("GLA_ArmsDealer".to_string(), gla_arms_dealer);

    // USA Buildings
    let mut usa_command = ThingTemplate::new("USA_CommandCenter");
    usa_command
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(2000.0)
        .set_cost(2000, -3)
        .set_model("abbtcmdhq"); // USA Command Center - correct model name
    templates.insert("USA_CommandCenter".to_string(), usa_command);

    let mut usa_barracks = ThingTemplate::new("USA_Barracks");
    usa_barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0)
        .set_cost(600, -1)
        .set_model("abbarracks"); // FactionBuilding.ini pristine USA barracks
    templates.insert("USA_Barracks".to_string(), usa_barracks);

    let mut usa_supply = ThingTemplate::new("USA_SupplyCenter");
    usa_supply
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(1000.0)
        .set_cost(1000, -1)
        .set_model("absupplyct"); // FactionBuilding.ini pristine USA supply center
    templates.insert("USA_SupplyCenter".to_string(), usa_supply);

    let mut usa_war_factory = ThingTemplate::new("USA_WarFactory");
    usa_war_factory
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0)
        .set_cost(1000, -2)
        .set_model("abwarfact"); // FactionBuilding.ini pristine USA war factory
    templates.insert("USA_WarFactory".to_string(), usa_war_factory);

    let mut usa_power = ThingTemplate::new("USA_PowerPlant");
    usa_power
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::PowerPlant)
        .set_health(800.0)
        .set_cost(800, 0) // Provides power, doesn't consume
        .set_model("abpwrplant"); // FactionBuilding.ini pristine USA power plant
    templates.insert("USA_PowerPlant".to_string(), usa_power);

    let mut usa_patriot = ThingTemplate::new("USA_Patriot");
    usa_patriot
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(600.0)
        .set_cost(1000, -2)
        .set_model("abpatriot") // FactionBuilding.ini pristine USA Patriot battery
        // Residual auto-fire: bind retail Patriot primary + AA secondary.
        .set_primary_weapon_name(super::weapon_bootstrap::PATRIOT_PRIMARY_WEAPON)
        .set_secondary_weapon_name(super::weapon_bootstrap::PATRIOT_SECONDARY_WEAPON);
    templates.insert("USA_Patriot".to_string(), usa_patriot);

    // China Buildings
    let mut china_command = ThingTemplate::new("China_CommandCenter");
    china_command
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(2200.0)
        .set_cost(2000, -3)
        .set_model("nbconyard"); // FactionBuilding.ini pristine China command center
    templates.insert("China_CommandCenter".to_string(), china_command);

    let mut china_barracks = ThingTemplate::new("China_Barracks");
    china_barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1100.0)
        .set_cost(500, -1)
        .set_model("nbbarracks"); // FactionBuilding.ini pristine China barracks
    templates.insert("China_Barracks".to_string(), china_barracks);

    let mut china_supply = ThingTemplate::new("China_SupplyCenter");
    china_supply
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(1000.0)
        .set_cost(1000, -1)
        .set_model("nbsupcent"); // FactionBuilding.ini pristine China supply center
    templates.insert("China_SupplyCenter".to_string(), china_supply);

    let mut china_war_factory = ThingTemplate::new("China_WarFactory");
    china_war_factory
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1400.0)
        .set_cost(1000, -2)
        .set_model("nbwarfact"); // FactionBuilding.ini pristine China war factory
    templates.insert("China_WarFactory".to_string(), china_war_factory);

    let mut china_power = ThingTemplate::new("China_PowerPlant");
    china_power
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::PowerPlant)
        .set_health(900.0)
        .set_cost(800, 0) // Provides power
        .set_model("nbpwrplant"); // FactionBuilding.ini pristine China power plant
    templates.insert("China_PowerPlant".to_string(), china_power);

    let mut china_gattling = ThingTemplate::new("China_GattlingCannon");
    china_gattling
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(500.0)
        .set_cost(800, -2)
        .set_model("nbgattling") // FactionBuilding.ini pristine China Gattling cannon
        // Residual auto-fire: bind retail Gattling building primary + AA secondary.
        .set_primary_weapon_name(super::weapon_bootstrap::GATTLING_BUILDING_PRIMARY_WEAPON)
        .set_secondary_weapon_name(super::weapon_bootstrap::GATTLING_BUILDING_SECONDARY_WEAPON);
    templates.insert("China_GattlingCannon".to_string(), china_gattling);

    // Additional GLA Buildings
    let mut gla_stinger = ThingTemplate::new("GLA_StingerSite");
    gla_stinger
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(400.0)
        .set_cost(800, -2)
        .set_model("ubstingers") // GLA Stinger Site - anti-air defense
        // SPAWNS_ARE_THE_WEAPONS residual: structure fires soldier weapons.
        .set_primary_weapon_name(super::weapon_bootstrap::STINGER_PRIMARY_WEAPON)
        .set_secondary_weapon_name(super::weapon_bootstrap::STINGER_SECONDARY_WEAPON);
    templates.insert("GLA_StingerSite".to_string(), gla_stinger);

    let mut gla_tunnel = ThingTemplate::new("GLA_TunnelNetwork");
    gla_tunnel
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(600.0)
        .set_cost(800, -1)
        .set_model("ubundtunn"); // FactionBuilding.ini pristine GLA tunnel network
    templates.insert("GLA_TunnelNetwork".to_string(), gla_tunnel);

    templates
}

/// Building behavior system
pub struct BuildingBehavior;

impl BuildingBehavior {
    /// Update building production
    pub fn update_production(
        object_id: ObjectId,
        objects: &mut HashMap<ObjectId, Object>,
        game_logic: &mut GameLogic,
        dt: f32,
    ) {
        // Prefer authoritative simulation state from GameLogic when available.
        let completion = if let Some(building) = game_logic.host_object_mut(object_id) {
            if !building.is_constructed() || !building.is_alive() {
                None
            } else if let Some(building_data) = building.building_data.as_mut() {
                let completed = building_data.update_production(dt, 1.0); // fallback path; main loop handles power
                let rally = building_data.rally_point;
                match completed {
                    Some(ProductionCompletion {
                        template_name,
                        kind: ProductionKind::Unit,
                        quantity,
                    }) => {
                        let spawn_pos = building.get_position()
                            + building.thing.get_direction_vector()
                                * building.selection_radius.max(10.0);
                        Some((building.team, template_name, spawn_pos, rally, quantity))
                    }
                    // PRODUCTION_UPGRADE residual is applied by GameLogic::update_production.
                    Some(ProductionCompletion {
                        kind: ProductionKind::Upgrade,
                        ..
                    })
                    | None => None,
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some((team, template_name, spawn_pos, rally_point, quantity)) = completion {
            for _ in 0..quantity {
                if let Some(new_id) = game_logic.create_object(&template_name, team, spawn_pos) {
                    if let Some(rally) = rally_point {
                        // Residual BuildingBehavior path — host update_production already
                        // path_approach_with_state; keep pathfind parity here too.
                        if !game_logic.assign_unit_path(new_id, rally, &[]) {
                            if let Some(unit) = game_logic.host_object_mut(new_id) {
                                unit.set_destination(rally);
                                unit.ai_state = AIState::Moving;
                            }
                        } else if let Some(unit) = game_logic.host_object_mut(new_id) {
                            unit.ai_state = AIState::Moving;
                        }
                    }
                }
            }
            return;
        }

        // Fallback for detached object maps used by isolated tests/tools.
        if let Some(building) = objects.get_mut(&object_id) {
            if let Some(building_data) = building.building_data.as_mut() {
                let _ = building_data.update_production(dt, 1.0); // fallback; no power context
            }
        }
    }

    /// Handle garrison mechanics
    pub fn garrison_unit(
        building_id: ObjectId,
        unit_id: ObjectId,
        objects: &mut HashMap<ObjectId, Object>,
    ) -> bool {
        let occupant_snap = objects.get(&unit_id).map(|unit| {
            (
                unit.team,
                unit.owner_player_id,
                unit.is_kind_of(KindOf::StealthGarrison),
                unit.status.detected,
            )
        });
        let can_garrison = if let (Some(building), Some(unit)) =
            (objects.get(&building_id), objects.get(&unit_id))
        {
            building.is_alive()
                && building.is_constructed()
                && building.can_contain()
                && building.garrison_container_accepts_entry()
                && unit.is_alive()
                && unit.is_kind_of(KindOf::Infantry)
                && !unit.is_kind_of(KindOf::NoGarrison)
                && building.get_position().distance(unit.get_position()) < 20.0
        } else {
            false
        };

        if !can_garrison {
            return false;
        }

        if let Some(building) = objects.get_mut(&building_id) {
            if !building.add_occupant(unit_id) {
                return false;
            }
        } else {
            return false;
        }

        if let Some(unit) = objects.get_mut(&unit_id) {
            unit.deselect();
            unit.stop();
            unit.target = Some(building_id);
            unit.contained_by = Some(building_id);
            unit.ai_state = AIState::Garrisoned;
            unit.status.moving = false;
            unit.status.attacking = false;
        } else {
            if let Some(building) = objects.get_mut(&building_id) {
                let _ = building.remove_occupant(unit_id);
            }
            return false;
        }

        if let Some((team, owner, stealth_garrison, detected)) = occupant_snap {
            if let Some(building) = objects.get_mut(&building_id) {
                building.note_garrison_occupant_entered(team, owner, stealth_garrison, detected);
            }
        }
        true
    }

    /// Handle ungarrison mechanics
    pub fn ungarrison_unit(
        building_id: ObjectId,
        objects: &mut HashMap<ObjectId, Object>,
    ) -> Option<ObjectId> {
        let (unit_id, building_pos, forward) = {
            let building = objects.get_mut(&building_id)?;
            if !building.is_alive() || !building.is_constructed() {
                return None;
            }

            let candidate = if let Some(data) = &building.building_data {
                data.garrisoned_units.last().copied()
            } else {
                building.occupants.last().copied()
            }?;

            let _ = building.remove_occupant(candidate);
            (
                candidate,
                building.get_position(),
                building.thing.get_direction_vector().normalize_or_zero(),
            )
        };

        if let Some(unit) = objects.get_mut(&unit_id) {
            let exit_offset = forward * (unit.selection_radius + 6.0);
            unit.set_position(building_pos + exit_offset);
            unit.target = None;
            unit.contained_by = None;
            unit.ai_state = AIState::Idle;
            unit.status.moving = false;
            unit.status.attacking = false;
            Some(unit_id)
        } else {
            None
        }
    }

    /// C++ `GarrisonContain::update` effectively-dead sweep: `removeFromContain`
    /// → `onRemoving` → free FIREPOINT/STATION. Missing occupants (already
    /// destroyed) also drop their slot.
    pub fn sweep_dead_garrison_occupants(
        objects: &mut HashMap<ObjectId, Object>,
        current_frame: u32,
    ) -> Vec<ObjectId> {
        let mut dead: Vec<(ObjectId, ObjectId)> = Vec::new();
        for (&cid, container) in objects.iter() {
            if !container.is_garrison_contain() {
                continue;
            }
            for occ in container.contained_units() {
                let gone = objects.get(&occ).map(|o| !o.is_alive()).unwrap_or(true);
                if gone {
                    dead.push((cid, occ));
                }
            }
        }
        let mut dirty: Vec<ObjectId> = Vec::new();
        for (cid, occ) in dead {
            if let Some(container) = objects.get_mut(&cid) {
                if container.remove_occupant(occ) && !dirty.contains(&cid) {
                    dirty.push(cid);
                }
            }
            if let Some(occ_obj) = objects.get_mut(&occ) {
                // C++ GarrisonContain.cpp:912-913 HUGE_FRAME_IN_FUTURE.
                occ_obj.stamp_safe_occlusion_frame_huge(current_frame);
            }
        }
        dirty
    }

    /// Update defensive buildings (turrets, etc.)
    pub fn update_defense_behavior(object_id: ObjectId, objects: &mut HashMap<ObjectId, Object>) {
        let (position, team, range) = {
            if let Some(building) = objects.get(&object_id) {
                if !building.is_alive()
                    || !building.is_constructed()
                    || !building.is_kind_of(KindOf::Attackable)
                {
                    return;
                }
                (
                    building.get_position(),
                    building.team,
                    building.get_template().sight_range,
                )
            } else {
                return;
            }
        };

        // Find nearest enemy
        let mut nearest_enemy = None;
        let mut nearest_distance = range;

        for (&other_id, other_obj) in objects.iter() {
            if other_id != object_id
                && other_obj.team != team
                && other_obj.is_alive()
                && other_obj.is_kind_of(KindOf::Attackable)
            {
                let distance = position.distance(other_obj.get_position());
                if distance < nearest_distance {
                    nearest_distance = distance;
                    nearest_enemy = Some(other_id);
                }
            }
        }

        // Attack nearest enemy
        if let Some(enemy_id) = nearest_enemy {
            if let Some(building) = objects.get_mut(&object_id) {
                building.attack_target(enemy_id);
            }
        }
    }

    /// Calculate power output/consumption for buildings
    pub fn calculate_power_for_team(team: Team, objects: &HashMap<ObjectId, Object>) -> (i32, i32) {
        let mut power_produced = 0;
        let mut power_consumed = 0;

        for obj in objects.values() {
            if obj.team == team && obj.is_constructed() && obj.is_alive() {
                power_produced += obj.power_provided;
                power_consumed += obj.power_consumed.abs();
            }
        }

        (power_produced, power_consumed)
    }

    /// Check if a team has sufficient power
    pub fn has_sufficient_power(team: Team, objects: &HashMap<ObjectId, Object>) -> bool {
        let (produced, consumed) = Self::calculate_power_for_team(team, objects);
        produced >= consumed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn building_factory_uses_pristine_retail_model_names() {
        let templates = create_building_templates();
        let expected = [
            ("GLA_Command", "ubcmdhq"),
            ("GLA_Barracks", "ubbarracks"),
            ("GLA_SupplyStash", "ubsupply"),
            ("GLA_ArmsDealer", "ubarmdeal"),
            ("USA_Barracks", "abbarracks"),
            ("USA_SupplyCenter", "absupplyct"),
            ("USA_WarFactory", "abwarfact"),
            ("USA_PowerPlant", "abpwrplant"),
            ("USA_Patriot", "abpatriot"),
            ("China_CommandCenter", "nbconyard"),
            ("China_Barracks", "nbbarracks"),
            ("China_SupplyCenter", "nbsupcent"),
            ("China_WarFactory", "nbwarfact"),
            ("China_PowerPlant", "nbpwrplant"),
            ("China_GattlingCannon", "nbgattling"),
            ("GLA_TunnelNetwork", "ubundtunn"),
        ];

        for (template_name, expected_model) in expected {
            assert_eq!(
                templates
                    .get(template_name)
                    .and_then(|template| template.model_name.as_deref()),
                Some(expected_model),
                "{template_name} must retain its pristine FactionBuilding.ini W3D model"
            );
        }
    }

    #[test]
    fn production_progress_is_clamped_to_valid_percent() {
        let mut building = BuildingData::new(BuildingType::Barracks);
        building.production_queue.push(ProductionItem {
            template_name: "TestInfantry".to_string(),
            progress: 12.0,
            total_time: 10.0,
            construction_frames: 0,
            cost: Resources::default(),
            quantity_total: 1,
            quantity_produced: 0,
            kind: ProductionKind::Unit,
        });

        assert_eq!(building.get_production_progress(), Some(1.0));

        building.production_queue[0].progress = -1.0;

        assert_eq!(building.get_production_progress(), Some(0.0));
    }

    #[test]
    fn zero_time_production_progress_reports_complete() {
        let mut building = BuildingData::new(BuildingType::Barracks);
        building.production_queue.push(ProductionItem {
            template_name: "TestInfantry".to_string(),
            progress: 0.0,
            total_time: 0.0,
            construction_frames: 0,
            cost: Resources::default(),
            quantity_total: 1,
            quantity_produced: 0,
            kind: ProductionKind::Unit,
        });

        assert_eq!(building.get_production_progress(), Some(1.0));
    }

    #[test]
    fn gla_arms_dealer_is_war_factory_for_vehicle_production() {
        assert_eq!(
            BuildingType::from_template_name("GLA_ArmsDealer"),
            BuildingType::WarFactory
        );
        assert_eq!(
            BuildingType::from_template_name("GLA Arms Dealer"),
            BuildingType::WarFactory
        );
        let bd = BuildingData::new(BuildingType::from_template_name("GLA_ArmsDealer"));
        let mut technical = ThingTemplate::new("GLA_Technical");
        technical.add_kind_of(KindOf::Vehicle);
        assert!(
            bd.can_produce(&technical),
            "ArmsDealer must produce vehicles"
        );
    }

    /// C++ BuildAssistant::isPossibleToMakeUnit only allows UNIT_BUILD
    /// templates on the barracks command set (KINDOF_INFANTRY). A unit whose
    /// name contains "Ranger" but is not infantry must not queue.
    #[test]
    fn barracks_can_produce_requires_infantry_kindof() {
        let bd = BuildingData::new(BuildingType::Barracks);
        let mut ranger = ThingTemplate::new("USA_Ranger");
        ranger.add_kind_of(KindOf::Infantry);
        assert!(
            bd.can_produce(&ranger),
            "shipped can_produce must accept infantry ranger"
        );
        let mut named_tank = ThingTemplate::new("AmericaRangerTank");
        named_tank.add_kind_of(KindOf::Vehicle);
        assert!(
            !bd.can_produce(&named_tank),
            "name-contains ranger is not a C++ factory gate"
        );
        let mut rebel = ThingTemplate::new("GLARebel");
        assert!(
            !bd.can_produce(&rebel),
            "missing KINDOF_INFANTRY must fail can_produce"
        );
    }

    #[test]
    fn free_garrison_point_for_clears_only_that_occupant() {
        let mut bd = BuildingData::new(BuildingType::Bunker);
        let a = ObjectId(10);
        let b = ObjectId(11);
        bd.garrison_point_occupant = vec![Some(a), Some(b), None];
        bd.free_garrison_point_for(a);
        assert_eq!(
            bd.garrison_point_occupant,
            vec![None, Some(b), None],
            "C++ onRemoving frees this occupant's FIREPOINT, not the whole ring"
        );
    }

    #[test]
    fn remove_occupant_frees_firepoint_while_survivors_keep_theirs() {
        use crate::game_logic::{ContainModuleKind, ContainModuleMetadata};
        let mut t = ThingTemplate::new("CivBunker");
        t.add_kind_of(KindOf::Structure);
        t.set_health(500.0);
        t.contain_module = ContainModuleMetadata {
            kind: ContainModuleKind::Garrison,
            slots: Some(10),
            ..ContainModuleMetadata::default()
        };
        let mut bunker = Object::new(t, ObjectId(1), Team::Neutral);
        let mut bd = BuildingData::new(BuildingType::Bunker);
        let dead = ObjectId(10);
        let survivor = ObjectId(11);
        bd.garrisoned_units = vec![dead, survivor];
        bd.garrison_point_occupant = vec![Some(dead), Some(survivor)];
        bunker.building_data = Some(bd);
        assert!(bunker.remove_occupant(dead));
        let bd = bunker.building_data.as_ref().unwrap();
        assert_eq!(bd.garrisoned_units, vec![survivor]);
        assert_eq!(
            bd.garrison_point_occupant,
            vec![None, Some(survivor)],
            "survivor must keep their window after a death/exit"
        );
    }

    #[test]
    fn sweep_dead_garrison_occupants_frees_missing_and_dead() {
        use crate::game_logic::{ContainModuleKind, ContainModuleMetadata};
        let mut bunker_t = ThingTemplate::new("CivBunker");
        bunker_t.add_kind_of(KindOf::Structure);
        bunker_t.set_health(500.0);
        bunker_t.contain_module = ContainModuleMetadata {
            kind: ContainModuleKind::Garrison,
            slots: Some(10),
            ..ContainModuleMetadata::default()
        };
        let mut bunker = Object::new(bunker_t, ObjectId(1), Team::Neutral);
        let mut ranger_t = ThingTemplate::new("AmericaRanger");
        ranger_t.add_kind_of(KindOf::Infantry);
        ranger_t.set_health(120.0);
        let mut dead = Object::new(ranger_t.clone(), ObjectId(10), Team::USA);
        dead.health.current = 0.0;
        dead.status.effectively_dead = true;
        let live = Object::new(ranger_t, ObjectId(11), Team::USA);
        let mut bd = BuildingData::new(BuildingType::Bunker);
        bd.garrisoned_units = vec![ObjectId(10), ObjectId(11), ObjectId(99)];
        bd.garrison_point_occupant =
            vec![Some(ObjectId(10)), Some(ObjectId(11)), Some(ObjectId(99))];
        bunker.building_data = Some(bd);
        let mut objects = HashMap::new();
        objects.insert(bunker.id, bunker);
        objects.insert(dead.id, dead);
        objects.insert(live.id, live);
        BuildingBehavior::sweep_dead_garrison_occupants(&mut objects, 0);
        let bd = objects
            .get(&ObjectId(1))
            .unwrap()
            .building_data
            .as_ref()
            .unwrap();
        assert_eq!(bd.garrisoned_units, vec![ObjectId(11)]);
        assert_eq!(
            bd.garrison_point_occupant,
            vec![None, Some(ObjectId(11)), None]
        );
        assert_eq!(
            objects.get(&ObjectId(10)).unwrap().safe_occlusion_frame,
            30_000,
            "dead garrison occupant must stamp HUGE_FRAME_IN_FUTURE"
        );
    }
}
