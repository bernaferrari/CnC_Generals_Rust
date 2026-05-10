use super::*;
use serde::{Deserialize, Serialize};

/// C++ ProductionUpdateModuleData default MaxQueueEntries.
pub const DEFAULT_PRODUCTION_QUEUE_LIMIT: usize = 9;

/// Building-specific data and behaviors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildingData {
    pub building_type: BuildingType,
    pub production_queue: Vec<ProductionItem>,
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
        } else if lower.contains("warfactory") || lower.contains("war factory") {
            BuildingType::WarFactory
        } else if lower.contains("airfield") || lower.contains("air field") {
            BuildingType::Airfield
        } else if lower.contains("repair") {
            BuildingType::RepairPad
        } else if lower.contains("hospital") || lower.contains("heal") || lower.contains("medic") {
            BuildingType::HealPad
        } else if lower.contains("supply") {
            BuildingType::SupplyCenter
        } else if lower.contains("power") {
            BuildingType::PowerPlant
        } else if lower.contains("patri") || lower.contains("turret") || lower.contains("defense") {
            BuildingType::DefenseTurret
        } else if lower.contains("dropzone") || lower.contains("drop zone") {
            BuildingType::SupplyDropZone
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionItem {
    pub template_name: String,
    pub progress: f32,
    pub total_time: f32,
    pub cost: Resources,
}

impl BuildingData {
    pub fn new(building_type: BuildingType) -> Self {
        let (power_output, power_requirement, max_garrison) = match building_type {
            BuildingType::CommandCenter => (0, -3, 0),
            BuildingType::Barracks => (0, -1, 10),
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
            BuildingType::Bunker => (0, 0, 20),
        };

        Self {
            building_type,
            production_queue: Vec::new(),
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
        if self.can_produce(template)
            && self.production_queue.len() < DEFAULT_PRODUCTION_QUEUE_LIMIT
        {
            let item = ProductionItem {
                template_name,
                progress: 0.0,
                total_time: template.build_time,
                cost: template.build_cost,
            };
            self.production_queue.push(item);
            true
        } else {
            false
        }
    }

    /// C++ parity (ThingTemplate::calcTimeToBuild): when energy ratio < 1.0
    /// production speed is reduced.  The penalty is:
    ///   energy_short = (1.0 - ratio) * penalty_modifier
    ///   rate = max(1.0 - energy_short, MIN_SPEED)
    ///   if ratio < 1.0: rate = min(rate, MAX_SPEED)
    /// Defaults: MIN=0.5, MAX=0.8, modifier=1.0  (GameData.ini).
    pub fn update_production(&mut self, dt: f32, power_factor: f32) -> Option<String> {
        let effective_dt = dt * power_factor.max(0.01);
        if let Some(item) = self.production_queue.first_mut() {
            item.progress += effective_dt;
            if item.progress >= item.total_time {
                // Production complete
                let completed_item = self.production_queue.remove(0);
                return Some(completed_item.template_name);
            }
        }
        None
    }

    pub fn cancel_production(&mut self, index: usize) -> Option<ProductionItem> {
        if index < self.production_queue.len() {
            Some(self.production_queue.remove(index))
        } else {
            None
        }
    }

    pub fn get_production_progress(&self) -> Option<f32> {
        self.production_queue
            .first()
            .map(|item| item.progress / item.total_time)
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
        .set_health(600.0)
        .set_cost(1000, -2)
        .set_model("abpatriotsw"); // USA patriot missile system model
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
        .set_health(500.0)
        .set_cost(800, -2)
        .set_model("nbgattling_a1"); // China gattling cannon model
    templates.insert("China_GattlingCannon".to_string(), china_gattling);

    // Additional GLA Buildings
    let mut gla_stinger = ThingTemplate::new("GLA_StingerSite");
    gla_stinger
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0)
        .set_cost(800, -2)
        .set_model("ubstingers"); // GLA Stinger Site - anti-air defense
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
                if let Some(template_name) = completed {
                    let spawn_pos = building.get_position()
                        + building.thing.get_direction_vector()
                            * building.selection_radius.max(10.0);
                    Some((building.team, template_name, spawn_pos, rally))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some((team, template_name, spawn_pos, rally_point)) = completion {
            if let Some(new_id) = game_logic.create_object(&template_name, team, spawn_pos) {
                if let Some(unit) = game_logic.find_object_mut(new_id) {
                    if let Some(rally) = rally_point {
                        unit.set_destination(rally);
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
