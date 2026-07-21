use super::*;
use serde::{Deserialize, Serialize};

/// C++ ProductionUpdateModuleData default MaxQueueEntries.
pub const DEFAULT_PRODUCTION_QUEUE_LIMIT: usize = 9;

/// Building-specific data and behaviors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildingData {
    pub building_type: BuildingType,
    pub production_queue: Vec<ProductionItem>,
    /// C++ QueueProductionExitUpdate exit countdown residual (seconds).
    /// While > 0, factory cannot release the next completed unit residual.
    pub exit_delay_remaining: f32,
    pub rally_point: Option<Vec3>,
    pub power_output: i32,
    pub power_requirement: i32,
    pub garrisoned_units: Vec<ObjectId>,
    pub max_garrison: usize,
    /// C++ parity (OpenContainModuleData::m_damagePercentageToUnits): percentage
    /// of damage passed to contained units when this building is destroyed.
    /// 0.0 = no damage (default), 1.0 = full max-health damage.
    pub damage_percent_to_units: f32,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionItem {
    pub template_name: String,
    pub progress: f32,
    pub total_time: f32,
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
            exit_delay_remaining: 0.0,
            rally_point: None,
            power_output,
            power_requirement,
            garrisoned_units: Vec::new(),
            max_garrison,
            damage_percent_to_units: 0.0,
        }
    }

    pub fn can_produce(&self, template: &ThingTemplate) -> bool {
        match self.building_type {
            BuildingType::Barracks => template.is_kind_of(KindOf::Infantry),
            BuildingType::WarFactory => template.is_kind_of(KindOf::Vehicle),
            BuildingType::Airfield => template.is_kind_of(KindOf::Aircraft),
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
        if self.can_produce(template)
            && self.production_queue.len() < DEFAULT_PRODUCTION_QUEUE_LIMIT
        {
            let item = ProductionItem {
                template_name,
                progress: 0.0,
                total_time: template.build_time,
                cost: template.build_cost,
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
    ) -> Option<(String, ProductionKind)> {
        self.tick_exit_delay(dt);
        self.advance_production_progress(dt, power_factor);
        self.try_complete_production()
    }

    /// C++ QueueProductionExitUpdate door/exit residual.
    pub fn tick_exit_delay(&mut self, dt: f32) {
        if self.exit_delay_remaining > 0.0 {
            self.exit_delay_remaining = (self.exit_delay_remaining - dt).max(0.0);
        }
    }

    /// Advance head-of-queue build timer only (no completion/spawn).
    /// Under GameWorld production authority, shadow owns this advance.
    pub fn advance_production_progress(&mut self, dt: f32, power_factor: f32) {
        let effective_dt = dt * power_factor.max(0.01);
        if let Some(item) = self.production_queue.first_mut() {
            // Only advance build timer until the batch is fully produced residual.
            if item.quantity_produced == 0 {
                item.progress += effective_dt;
            }
            if item.progress > item.total_time {
                item.progress = item.total_time;
            }
        }
    }

    /// Complete/release head-of-queue when progress is done and exit delay clear.
    pub fn try_complete_production(&mut self) -> Option<(String, ProductionKind)> {
        if let Some(item) = self.production_queue.first_mut() {
            if item.progress >= item.total_time {
                // Hold each unit release until exit delay residual clears.
                if self.exit_delay_remaining > 0.0 {
                    // Clamp at complete so timer doesn't overshoot residual.
                    item.progress = item.total_time;
                    return None;
                }
                // Release one unit from this ProductionEntry residual.
                item.progress = item.total_time;
                item.quantity_produced = item.quantity_produced.saturating_add(1);
                let name = item.template_name.clone();
                let kind = item.kind;
                let done = item.quantity_produced >= item.quantity_total.max(1);
                if done {
                    self.production_queue.remove(0);
                }
                return Some((name, kind));
            }
        }
        None
    }

    /// Arm QueueProductionExitUpdate residual after a unit exits.
    pub fn arm_exit_delay(&mut self, delay_seconds: f32) {
        self.exit_delay_remaining = delay_seconds.max(0.0);
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
        .set_model("ubarfrccmd"); // GLA command center model
    templates.insert("GLA_Command".to_string(), gla_command);

    let mut gla_barracks = ThingTemplate::new("GLA_Barracks");
    gla_barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0)
        .set_cost(500, -1)
        .set_model("ubbarracksf"); // GLA barracks model
    templates.insert("GLA_Barracks".to_string(), gla_barracks);

    let mut gla_supply = ThingTemplate::new("GLA_SupplyStash");
    gla_supply
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(600.0)
        .set_cost(800, -1)
        .set_model("ubsupply_f"); // GLA supply stash model
    templates.insert("GLA_SupplyStash".to_string(), gla_supply);

    let mut gla_arms_dealer = ThingTemplate::new("GLA_ArmsDealer");
    gla_arms_dealer
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1200.0)
        .set_cost(800, -2)
        .set_model("ubarmdealf"); // GLA arms dealer model
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
        .set_model("abbarracks_fa"); // USA barracks model
    templates.insert("USA_Barracks".to_string(), usa_barracks);

    let mut usa_supply = ThingTemplate::new("USA_SupplyCenter");
    usa_supply
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(1000.0)
        .set_cost(1000, -1)
        .set_model("absupplyct_a2"); // USA supply center model
    templates.insert("USA_SupplyCenter".to_string(), usa_supply);

    let mut usa_war_factory = ThingTemplate::new("USA_WarFactory");
    usa_war_factory
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0)
        .set_cost(1000, -2)
        .set_model("abwarfact_e"); // USA war factory model
    templates.insert("USA_WarFactory".to_string(), usa_war_factory);

    let mut usa_power = ThingTemplate::new("USA_PowerPlant");
    usa_power
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::PowerPlant)
        .set_health(800.0)
        .set_cost(800, 0) // Provides power, doesn't consume
        .set_model("abpwrplant_d06"); // USA power plant model
    templates.insert("USA_PowerPlant".to_string(), usa_power);

    let mut usa_patriot = ThingTemplate::new("USA_Patriot");
    usa_patriot
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(600.0)
        .set_cost(1000, -2)
        .set_model("abpatriotsw") // USA patriot missile system model
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
        .set_model("nbconyard_fa"); // China command center model
    templates.insert("China_CommandCenter".to_string(), china_command);

    let mut china_barracks = ThingTemplate::new("China_Barracks");
    china_barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1100.0)
        .set_cost(500, -1)
        .set_model("nbintcnt"); // China infantry center model
    templates.insert("China_Barracks".to_string(), china_barracks);

    let mut china_supply = ThingTemplate::new("China_SupplyCenter");
    china_supply
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(1000.0)
        .set_cost(1000, -1)
        .set_model("cxsupcent"); // Supply center model used by China build flow
    templates.insert("China_SupplyCenter".to_string(), china_supply);

    let mut china_war_factory = ThingTemplate::new("China_WarFactory");
    china_war_factory
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1400.0)
        .set_cost(1000, -2)
        .set_model("nbweapfact"); // China war factory model
    templates.insert("China_WarFactory".to_string(), china_war_factory);

    let mut china_power = ThingTemplate::new("China_PowerPlant");
    china_power
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::PowerPlant)
        .set_health(900.0)
        .set_cost(800, 0) // Provides power
        .set_model("nbnreactr"); // China nuclear reactor model
    templates.insert("China_PowerPlant".to_string(), china_power);

    let mut china_gattling = ThingTemplate::new("China_GattlingCannon");
    china_gattling
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(500.0)
        .set_cost(800, -2)
        .set_model("nbgattling_a1") // China gattling cannon model
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
        .set_model("ubhole_a4"); // GLA tunnel network model
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
        let completion = if let Some(building) = game_logic.find_object_mut(object_id) {
            if !building.is_constructed() || !building.is_alive() {
                None
            } else if let Some(building_data) = building.building_data.as_mut() {
                let completed = building_data.update_production(dt, 1.0); // fallback path; main loop handles power
                let rally = building_data.rally_point;
                match completed {
                    Some((template_name, ProductionKind::Unit)) => {
                        let spawn_pos = building.get_position()
                            + building.thing.get_direction_vector()
                                * building.selection_radius.max(10.0);
                        Some((building.team, template_name, spawn_pos, rally))
                    }
                    // PRODUCTION_UPGRADE residual is applied by GameLogic::update_production.
                    Some((_, ProductionKind::Upgrade)) | None => None,
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some((team, template_name, spawn_pos, rally_point)) = completion {
            if let Some(new_id) = game_logic.create_object(&template_name, team, spawn_pos) {
                if let Some(rally) = rally_point {
                    // Residual BuildingBehavior path — host update_production already
                    // path_approach_with_state; keep pathfind parity here too.
                    if !game_logic.assign_unit_path(new_id, rally, &[]) {
                        if let Some(unit) = game_logic.find_object_mut(new_id) {
                            unit.set_destination(rally);
                            unit.ai_state = AIState::Moving;
                        }
                    } else if let Some(unit) = game_logic.find_object_mut(new_id) {
                        unit.ai_state = AIState::Moving;
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
        let can_garrison = if let (Some(building), Some(unit)) =
            (objects.get(&building_id), objects.get(&unit_id))
        {
            building.is_alive()
                && building.is_constructed()
                && building.can_contain()
                && unit.is_alive()
                && unit.is_kind_of(KindOf::Infantry)
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
            true
        } else {
            if let Some(building) = objects.get_mut(&building_id) {
                let _ = building.remove_occupant(unit_id);
            }
            false
        }
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
    fn production_progress_is_clamped_to_valid_percent() {
        let mut building = BuildingData::new(BuildingType::Barracks);
        building.production_queue.push(ProductionItem {
            template_name: "TestInfantry".to_string(),
            progress: 12.0,
            total_time: 10.0,
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
}
