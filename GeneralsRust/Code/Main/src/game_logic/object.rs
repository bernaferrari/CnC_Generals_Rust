use super::*;
use crate::command_system::SpecialPowerType;
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Object type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectType {
    Infantry,
    Vehicle,
    Aircraft,
    Building,
    Supply,
    Projectile,
    Neutral,
}

/// Game Object - the main entity class for all game units, buildings, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    /// Base Thing functionality
    pub thing: Thing,

    /// Unique identifier
    pub id: ObjectId,

    /// Team ownership
    pub team: Team,

    /// Object name
    pub name: String,

    /// Object status
    pub status: ObjectStatus,

    /// Health system
    pub health: Health,

    /// Movement system
    pub movement: Movement,

    /// Experience system
    pub experience: Experience,

    /// Primary weapon
    pub weapon: Option<Weapon>,

    /// Current target
    pub target: Option<ObjectId>,

    /// Construction progress (0.0 to 1.0)
    pub construction_percent: f32,

    /// Building-specific data (present for structures)
    pub building_data: Option<BuildingData>,

    /// Resource storage for buildings
    pub stored_resources: Resources,

    /// Power provided/consumed
    pub power_provided: i32,
    pub power_consumed: i32,

    /// Selection state
    pub selected: bool,

    /// AI state for autonomous behavior
    pub ai_state: AIState,

    // Command system compatibility fields
    /// Object type identifier
    pub object_type: ObjectType,

    /// Template name for identification
    pub template_name: String,

    /// Current position (shadow of thing.position for compatibility)
    pub position: Vec3,

    /// Maximum health
    pub max_health: f32,

    /// Target location for ground attacks
    pub target_location: Option<Vec3>,

    /// Guard position
    pub guard_position: Option<Vec3>,

    /// Guard target
    pub guard_target: Option<ObjectId>,

    /// Force attack mode
    pub force_attack: bool,

    /// Visual properties for rendering
    pub show_health_bar: bool,
    pub selection_radius: f32,
    pub team_color: [f32; 4],

    /// Tracked occupants for transports/garrisons
    pub occupants: Vec<ObjectId>,

    /// Optional short-lived cheer/animation timer
    pub cheer_timer: f32,

    /// Toggleable weapon/overcharge state flags
    pub overcharge_enabled: bool,
    pub active_weapon_slot: u8,

    /// Stored guard radius for pathing/AI persistence
    pub guard_radius: f32,

    /// Applied upgrades keyed by upgrade template/tag name.
    pub applied_upgrades: HashSet<String>,

    /// Special power availability/cooldown state.
    pub special_power_ready: bool,
    pub special_power_cooldown: f32,
    pub special_power_cooldown_remaining: f32,
}

/// AI behavior states
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AIState {
    Idle,
    Moving,
    Attacking,
    AttackMoving,
    AttackingGround,
    Gathering,
    ReturningResources,
    Constructing,
    Repairing,
    GuardingArea,
    GuardingObject,
    Patrolling,
    Docked,
    Garrisoned,
    SpecialAbility,
    SeekingRepair,
    SeekingHealing,
    Entering,
    Docking,
    Capturing,
}

impl Object {
    pub fn new(template: ThingTemplate, id: ObjectId, team: Team) -> Self {
        let max_health = template.max_health;
        let position = Vec3::ZERO; // Default position
        let template_name = template.name.clone();

        // Determine object type from template
        let object_type = if template.is_kind_of(KindOf::Infantry) {
            ObjectType::Infantry
        } else if template.is_kind_of(KindOf::Vehicle) {
            ObjectType::Vehicle
        } else if template.is_kind_of(KindOf::Aircraft) {
            ObjectType::Aircraft
        } else if template.is_kind_of(KindOf::Structure) {
            ObjectType::Building
        } else {
            ObjectType::Neutral
        };

        // Calculate selection radius based on object type
        let selection_radius = match object_type {
            ObjectType::Infantry => 8.0,
            ObjectType::Vehicle => 15.0,
            ObjectType::Aircraft => 20.0,
            ObjectType::Building => 25.0,
            ObjectType::Neutral => 10.0,
            _ => 10.0,
        };

        let building_data = if object_type == ObjectType::Building {
            let building_type = BuildingType::from_template_name(&template_name);
            Some(BuildingData::new(building_type))
        } else {
            None
        };

        let special_power_cooldown = template.special_power_cooldown;

        let (power_provided, power_consumed) = building_data
            .as_ref()
            .map(|data| (data.power_output, data.power_requirement))
            .unwrap_or((0, 0));

        Self {
            thing: Thing::new(template),
            id,
            team,
            name: String::new(),
            status: ObjectStatus::default(),
            health: Health::new(max_health),
            movement: Movement::default(),
            experience: Experience::default(),
            weapon: None,
            target: None,
            construction_percent: 1.0, // Fully constructed by default
            building_data,
            stored_resources: Resources::default(),
            power_provided,
            power_consumed,
            selected: false,
            ai_state: AIState::Idle,
            object_type,
            template_name,
            position,
            max_health,
            target_location: None,
            guard_position: None,
            guard_target: None,
            force_attack: false,
            show_health_bar: true, // Show health bars by default
            selection_radius,
            team_color: team.get_color(),
            occupants: Vec::new(),
            cheer_timer: 0.0,
            overcharge_enabled: false,
            active_weapon_slot: 0,
            guard_radius: 0.0,
            applied_upgrades: HashSet::new(),
            special_power_ready: true,
            special_power_cooldown,
            special_power_cooldown_remaining: 0.0,
        }
    }

    /// Alternative constructor for command system compatibility
    pub fn new_simple(id: ObjectId, object_type: ObjectType, template_name: String) -> Self {
        let template = ThingTemplate::new(&template_name);
        let team = Team::Neutral;
        let selection_radius = match object_type {
            ObjectType::Infantry => 8.0,
            ObjectType::Vehicle => 15.0,
            ObjectType::Aircraft => 20.0,
            ObjectType::Building => 25.0,
            ObjectType::Neutral => 10.0,
            _ => 10.0,
        };

        Self {
            thing: Thing::new(template),
            id,
            team,
            name: String::new(),
            status: ObjectStatus::default(),
            health: Health::new(100.0),
            movement: Movement::default(),
            experience: Experience::default(),
            weapon: None,
            target: None,
            construction_percent: 1.0,
            building_data: None,
            stored_resources: Resources::default(),
            power_provided: 0,
            power_consumed: 0,
            selected: false,
            ai_state: AIState::Idle,
            object_type,
            template_name,
            position: Vec3::ZERO,
            max_health: 100.0,
            target_location: None,
            guard_position: None,
            guard_target: None,
            force_attack: false,
            show_health_bar: true,
            selection_radius,
            team_color: team.get_color(),
            occupants: Vec::new(),
            cheer_timer: 0.0,
            overcharge_enabled: false,
            active_weapon_slot: 0,
            guard_radius: 0.0,
            applied_upgrades: HashSet::new(),
            special_power_ready: true,
            special_power_cooldown: 10.0,
            special_power_cooldown_remaining: 0.0,
        }
    }

    pub fn new_under_construction(template: ThingTemplate, id: ObjectId, team: Team) -> Self {
        let mut obj = Self::new(template, id, team);
        obj.construction_percent = 0.0;
        obj.status.under_construction = true;
        obj.health.current = 0.1; // Very low health during construction
        obj
    }

    pub fn get_template(&self) -> &ThingTemplate {
        self.thing.get_template()
    }

    pub fn is_kind_of(&self, kind: KindOf) -> bool {
        self.thing.is_kind_of(kind)
    }

    pub fn is_alive(&self) -> bool {
        !self.status.destroyed && self.health.is_alive()
    }

    pub fn is_constructed(&self) -> bool {
        !self.status.under_construction && self.construction_percent >= 1.0
    }

    pub fn is_mobile(&self) -> bool {
        self.is_kind_of(KindOf::Infantry)
            || self.is_kind_of(KindOf::Vehicle)
            || self.is_kind_of(KindOf::Aircraft)
    }

    pub fn is_selectable(&self) -> bool {
        self.is_alive()
            && self.is_kind_of(KindOf::Selectable)
            && !matches!(self.ai_state, AIState::Docked | AIState::Garrisoned)
    }

    pub fn is_worker(&self) -> bool {
        self.is_kind_of(KindOf::Worker)
            || self.template_name.contains("Dozer")
            || self.template_name.contains("Worker")
            || self.template_name.contains("Harvester")
            || self.template_name.contains("Collector")
    }

    pub fn is_hero(&self) -> bool {
        self.is_kind_of(KindOf::Hero) || self.template_name.contains("Hero")
    }

    pub fn is_command_center(&self) -> bool {
        self.is_kind_of(KindOf::CommandCenter)
            || self.template_name.contains("CommandCenter")
            || self.template_name.contains("Headquarters")
    }

    pub fn can_attack(&self) -> bool {
        self.is_alive()
            && self.weapon.is_some()
            && !self.status.disabled_underpowered
            && !matches!(
                self.ai_state,
                AIState::Docked | AIState::Garrisoned | AIState::Entering
            )
    }

    pub fn is_attackable(&self) -> bool {
        self.is_alive() && self.is_kind_of(KindOf::Attackable)
    }

    pub fn get_position(&self) -> Vec3 {
        self.thing.get_position()
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.thing.set_position(position);
    }

    pub fn get_orientation(&self) -> f32 {
        self.thing.get_orientation()
    }

    pub fn set_orientation(&mut self, angle: f32) {
        self.thing.set_orientation(angle);
    }

    pub fn get_transform_matrix(&self) -> Mat4 {
        self.thing.get_transform_matrix()
    }

    pub fn take_damage(&mut self, damage: f32) -> bool {
        if self.status.destroyed {
            return false;
        }

        // Apply armor reduction
        let armor_factor = 1.0 - (self.thing.template.armor / (self.thing.template.armor + 100.0));
        let actual_damage = damage * armor_factor;

        self.health.damage(actual_damage);

        // Check if object is destroyed
        if !self.health.is_alive() {
            self.status.destroyed = true;
            self.ai_state = AIState::Idle;
            self.target = None;
            true // Object was destroyed
        } else {
            false
        }
    }

    pub fn heal(&mut self, amount: f32) {
        if !self.status.destroyed {
            self.health.heal(amount);
        }
    }

    pub fn can_target(&self, target: &Object) -> bool {
        if let Some(weapon) = &self.weapon {
            let target_is_air =
                target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target;

            if target_is_air && !weapon.can_target_air {
                return false;
            }

            if !target_is_air && !weapon.can_target_ground {
                return false;
            }

            // Check range
            let distance = self.thing.get_distance_to(&target.thing);
            distance <= weapon.range
        } else {
            false
        }
    }

    pub fn can_fire(&self, current_time: f32) -> bool {
        if let Some(weapon) = &self.weapon {
            current_time - weapon.last_fire_time >= weapon.reload_time
        } else {
            false
        }
    }

    pub fn fire_at(&mut self, target_id: ObjectId, current_time: f32) -> bool {
        let can_fire = if let Some(weapon) = &self.weapon {
            current_time - weapon.last_fire_time >= weapon.reload_time
        } else {
            false
        };

        if can_fire {
            if let Some(weapon) = &mut self.weapon {
                weapon.last_fire_time = current_time;
                self.target = Some(target_id);

                // Extract data before calling self.get_position() to satisfy
                // the borrow checker (self.weapon is mutably borrowed here).
                let weapon_damage = weapon.damage;
                let shooter_id = self.id;
                let shooter_pos = self.get_position();

                // Queue a projectile for the combat system to spawn this frame.
                super::combat::queue_projectile(super::combat::PendingProjectile {
                    shooter_id,
                    shooter_pos,
                    target_id: Some(target_id),
                    target_pos: shooter_pos, // placeholder
                    damage: weapon_damage,
                    speed: 200.0,
                });

                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn move_to(&mut self, position: Vec3) {
        if self.is_mobile() && self.is_alive() {
            self.movement.target_position = Some(position);
            self.ai_state = AIState::Moving;
            self.status.moving = true;
        }
    }

    pub fn stop_moving(&mut self) {
        self.movement.target_position = None;
        self.movement.velocity = Vec3::ZERO;
        self.movement.path.clear();
        self.movement.current_path_index = 0;
        self.ai_state = AIState::Idle;
        self.status.moving = false;
    }

    pub fn attack_target(&mut self, target_id: ObjectId) {
        if self.can_attack() && self.is_alive() {
            self.target = Some(target_id);
            self.target_location = None;
            self.force_attack = false;
            self.ai_state = AIState::Attacking;
            self.status.attacking = true;
        }
    }

    pub fn stop_attack(&mut self) {
        self.target = None;
        self.target_location = None;
        self.force_attack = false;
        self.ai_state = AIState::Idle;
        self.status.attacking = false;
    }

    pub fn clear_all_occupants(&mut self) {
        if let Some(building) = self.building_data.as_mut() {
            building.garrisoned_units.clear();
        }
        self.occupants.clear();
    }

    // Command system compatibility methods
    pub fn can_move(&self) -> bool {
        self.is_mobile()
            && self.is_alive()
            && !matches!(self.ai_state, AIState::Docked | AIState::Garrisoned)
    }

    pub fn set_destination(&mut self, destination: Vec3) {
        self.move_to(destination);
    }

    pub fn set_target(&mut self, target: Option<ObjectId>) {
        self.target = target;
        if target.is_some() {
            self.target_location = None;
            self.ai_state = AIState::Attacking;
            self.status.attacking = true;
        } else {
            self.target_location = None;
            self.force_attack = false;
            self.ai_state = AIState::Idle;
            self.status.attacking = false;
        }
    }

    /// Check whether this object can fire the requested special power.
    pub fn is_special_power_ready(&self, _power: &SpecialPowerType) -> bool {
        self.is_alive() && self.special_power_ready && self.special_power_cooldown_remaining <= 0.0
    }

    /// Consume a charge for the special power and start cooldown.
    pub fn consume_special_power_charge(&mut self, power: &SpecialPowerType) {
        if !self.is_special_power_ready(power) {
            return;
        }
        self.special_power_ready = false;
        self.special_power_cooldown_remaining = self.special_power_cooldown;
        self.ai_state = AIState::Idle;
    }

    pub fn apply_upgrade_tag(&mut self, upgrade: &str) {
        if !upgrade.is_empty() {
            self.applied_upgrades.insert(upgrade.to_string());
        }
    }

    pub fn has_upgrade_tag(&self, upgrade: &str) -> bool {
        self.applied_upgrades.contains(upgrade)
    }

    pub fn set_target_location(&mut self, location: Option<Vec3>) {
        self.target_location = location;
        if location.is_some() {
            self.target = None;
            self.ai_state = AIState::Attacking;
            self.status.attacking = true;
        } else {
            self.force_attack = false;
        }
    }

    pub fn set_force_attack(&mut self, force: bool) {
        self.force_attack = force;
    }

    pub fn stop(&mut self) {
        // Stop all current actions
        self.stop_moving();
        self.stop_attack();
    }

    pub fn set_guard_position(&mut self, position: Option<Vec3>) {
        self.guard_position = position;
        if position.is_some() {
            self.ai_state = AIState::GuardingArea;
        }
    }

    pub fn set_guard_target(&mut self, target: Option<ObjectId>) {
        self.guard_target = target;
        if target.is_some() {
            self.ai_state = AIState::GuardingObject;
        }
    }

    pub fn can_repair(&self) -> bool {
        // Repair/build authority should be limited to worker/dozer-style units.
        self.can_move() && self.is_worker()
    }

    pub fn can_construct(&self) -> bool {
        // Construction should be limited to worker/dozer-style units.
        self.can_move() && self.is_worker()
    }

    pub fn can_contain(&self) -> bool {
        // Check if this object can contain other units (transport, garrison)
        self.is_alive() && (self.is_kind_of(KindOf::Structure) || self.is_kind_of(KindOf::Vehicle))
    }

    pub fn has_capacity_for(&self, count: usize) -> bool {
        if let Some(building) = &self.building_data {
            building.garrisoned_units.len() + count <= building.max_garrison
        } else {
            // Transport heuristic based on footprint: larger selection radius holds more
            let base_cap = (self.selection_radius / 8.0).ceil() as usize + 2;
            let max_cap = base_cap.clamp(2, 12);
            self.occupants.len() + count <= max_cap
        }
    }

    pub fn add_occupant(&mut self, unit_id: ObjectId) -> bool {
        if !self.can_contain() || !self.has_capacity_for(1) {
            return false;
        }
        if let Some(building) = self.building_data.as_mut() {
            if building.garrisoned_units.contains(&unit_id) {
                return true;
            }
            building.garrisoned_units.push(unit_id);
            true
        } else {
            if self.occupants.contains(&unit_id) {
                return true;
            }
            self.occupants.push(unit_id);
            true
        }
    }

    pub fn contained_units(&self) -> Vec<ObjectId> {
        if let Some(building) = &self.building_data {
            building.garrisoned_units.clone()
        } else {
            self.occupants.clone()
        }
    }

    pub fn remove_occupant(&mut self, unit_id: ObjectId) -> bool {
        if let Some(building) = self.building_data.as_mut() {
            if let Some(pos) = building
                .garrisoned_units
                .iter()
                .position(|&id| id == unit_id)
            {
                building.garrisoned_units.remove(pos);
                return true;
            }
        }
        if let Some(pos) = self.occupants.iter().position(|&id| id == unit_id) {
            self.occupants.remove(pos);
            return true;
        }
        false
    }

    /// Begin containing an occupant (transport/garrison bookkeeping).
    pub fn enter_transport(&mut self, unit_id: ObjectId) -> bool {
        self.add_occupant(unit_id)
    }

    /// Remove an occupant from this transport/garrison.
    pub fn exit_transport(&mut self, unit_id: ObjectId) -> bool {
        self.remove_occupant(unit_id)
    }

    pub fn tick_timers(&mut self, dt: f32) {
        if self.cheer_timer > 0.0 {
            self.cheer_timer -= dt;
            if self.cheer_timer <= 0.0 && self.ai_state == AIState::SpecialAbility {
                self.ai_state = AIState::Idle;
                self.cheer_timer = 0.0;
            }
        }

        if self.special_power_cooldown_remaining > 0.0 {
            self.special_power_cooldown_remaining =
                (self.special_power_cooldown_remaining - dt).max(0.0);
            if self.special_power_cooldown_remaining <= 0.0 {
                self.special_power_ready = true;
            }
        }
    }

    pub fn update_construction(&mut self, dt: f32) {
        if self.status.under_construction {
            let build_rate = 1.0 / self.thing.template.build_time;
            self.construction_percent += build_rate * dt;

            if self.construction_percent >= 1.0 {
                self.construction_percent = 1.0;
                self.status.under_construction = false;
                self.health.current = self.health.maximum;
            } else {
                // Health scales with construction progress
                self.health.current = self.health.maximum * (0.1 + 0.9 * self.construction_percent);
            }
        }
    }

    pub fn update_movement(&mut self, dt: f32) {
        if matches!(self.ai_state, AIState::Docked | AIState::Garrisoned) {
            self.movement.target_position = None;
            self.movement.velocity = Vec3::ZERO;
            return;
        }

        if let Some(target_pos) = self.movement.target_position {
            let current_pos = self.get_position();
            let direction = (target_pos - current_pos).normalize_or_zero();

            if direction.length() > 0.0 {
                // Update velocity
                let target_velocity = direction * self.movement.max_speed;
                let velocity_diff = target_velocity - self.movement.velocity;
                let max_accel = self.movement.acceleration * dt;

                if velocity_diff.length() <= max_accel {
                    self.movement.velocity = target_velocity;
                } else {
                    self.movement.velocity += velocity_diff.normalize() * max_accel;
                }

                // Update position
                let new_position = current_pos + self.movement.velocity * dt;
                self.set_position(new_position);

                // Update orientation to face movement direction
                if self.movement.velocity.length() > 0.1 {
                    let desired_angle = (-self.movement.velocity.z).atan2(self.movement.velocity.x);
                    let current_angle = self.get_orientation();
                    let angle_diff = desired_angle - current_angle;

                    // Normalize angle difference
                    let angle_diff = ((angle_diff + std::f32::consts::PI)
                        % (2.0 * std::f32::consts::PI))
                        - std::f32::consts::PI;

                    let max_turn = self.movement.turn_rate * dt;
                    let new_angle = if angle_diff.abs() <= max_turn {
                        desired_angle
                    } else {
                        current_angle + max_turn * angle_diff.signum()
                    };

                    self.set_orientation(new_angle);
                }

                // Check if we've reached the target
                let distance_to_target = current_pos.distance(target_pos);
                if distance_to_target < 2.0 {
                    self.stop_moving();
                }
            } else {
                self.stop_moving();
            }
        }
    }

    pub fn gain_experience(&mut self, amount: f32) {
        self.experience.current += amount;

        // Check for level up
        let previous_level = self.experience.level;
        let new_level = match self.experience.current {
            x if x >= 300.0 => VeterancyLevel::Heroic,
            x if x >= 150.0 => VeterancyLevel::Elite,
            x if x >= 60.0 => VeterancyLevel::Veteran,
            _ => VeterancyLevel::Rookie,
        };

        if new_level != previous_level {
            self.experience.level = new_level;
            // Apply veterancy bonuses
            self.apply_veterancy_bonuses(previous_level, new_level);
        }
    }

    fn veterancy_bonuses(level: VeterancyLevel) -> (f32, f32) {
        match level {
            VeterancyLevel::Rookie => (1.0, 1.0),
            VeterancyLevel::Veteran => (1.25, 1.25),
            VeterancyLevel::Elite => (1.5, 1.5),
            VeterancyLevel::Heroic => (2.0, 2.0),
        }
    }

    fn apply_veterancy_bonuses(
        &mut self,
        previous_level: VeterancyLevel,
        new_level: VeterancyLevel,
    ) {
        let (_old_health_bonus, old_damage_bonus) = Self::veterancy_bonuses(previous_level);
        let (health_bonus, damage_bonus) = Self::veterancy_bonuses(new_level);

        // Apply health bonus
        let base_health = self.thing.template.max_health;
        let old_max_health = self.health.maximum.max(1.0);
        let health_ratio = (self.health.current / old_max_health).clamp(0.0, 1.0);
        self.health.maximum = base_health * health_bonus;
        self.health.current = (self.health.maximum * health_ratio).clamp(0.0, self.health.maximum);

        // Apply weapon damage bonus
        if let Some(weapon) = &mut self.weapon {
            let scale = if old_damage_bonus > 0.0 {
                damage_bonus / old_damage_bonus
            } else {
                1.0
            };
            weapon.damage *= scale;
        }
    }

    pub fn select(&mut self) {
        if self.is_selectable() {
            self.selected = true;
            self.status.selected = true;
        }
    }

    pub fn deselect(&mut self) {
        self.selected = false;
        self.status.selected = false;
    }

    /// Set the AI state for autonomous behavior
    pub fn set_ai_state(&mut self, state: AIState) {
        self.ai_state = state;
    }

    /// Get visual information for rendering
    pub fn get_visual_info(&self) -> ObjectVisualInfo {
        ObjectVisualInfo {
            position: self.get_position(),
            orientation: self.get_orientation(),
            team_color: self.team_color,
            selection_radius: self.selection_radius,
            is_selected: self.selected,
            show_health_bar: self.show_health_bar && self.is_alive(),
            health_percentage: self.health.percentage(),
            model_name: self.thing.template.model_name.clone(),
            object_type: self.object_type,
            team: self.team,
            under_construction: self.status.under_construction,
            construction_percent: self.construction_percent,
        }
    }

    /// Update team color (useful for changing allegiance)
    pub fn set_team(&mut self, team: Team) {
        self.team = team;
        self.team_color = team.get_color();
    }

    /// Check if this object is visible to a team (for fog of war)
    pub fn is_visible_to_team(&self, team: Team) -> bool {
        // Team-local baseline visibility check. Global shroud/fog filtering is applied by
        // higher-level visibility queries in GameLogic that have object IDs and player context.
        !self.status.stealthed || team == self.team
    }

    /// Get a description string for UI display
    pub fn get_display_name(&self) -> String {
        if self.name.is_empty() {
            self.template_name.clone()
        } else {
            self.name.clone()
        }
    }
}

/// Visual information structure for rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectVisualInfo {
    pub position: Vec3,
    pub orientation: f32,
    pub team_color: [f32; 4],
    pub selection_radius: f32,
    pub is_selected: bool,
    pub show_health_bar: bool,
    pub health_percentage: f32,
    pub model_name: Option<String>,
    pub object_type: ObjectType,
    pub team: Team,
    pub under_construction: bool,
    pub construction_percent: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_object() -> Object {
        let template = ThingTemplate::new("TestUnit");
        let mut object = Object::new(template, ObjectId(1), Team::USA);
        object.weapon = Some(Weapon {
            damage: 100.0,
            ..Weapon::default()
        });
        object
    }

    #[test]
    fn veterancy_increases_weapon_damage() {
        let mut object = make_test_object();
        object.gain_experience(60.0); // Veteran
        let veteran_damage = object.weapon.as_ref().map(|w| w.damage).unwrap_or_default();
        assert!((veteran_damage - 125.0).abs() < 0.01);

        object.gain_experience(90.0); // Elite
        let elite_damage = object.weapon.as_ref().map(|w| w.damage).unwrap_or_default();
        assert!((elite_damage - 150.0).abs() < 0.01);
    }

    #[test]
    fn veterancy_preserves_health_ratio_when_max_health_changes() {
        let mut object = make_test_object();
        object.health.current = 50.0;
        object.health.maximum = 100.0;

        object.gain_experience(60.0); // Veteran
        assert!((object.health.maximum - 125.0).abs() < 0.01);
        assert!((object.health.current - 62.5).abs() < 0.01);
    }

    #[test]
    fn stop_attack_clears_force_attack_and_targets() {
        let mut object = make_test_object();
        object.set_target(Some(ObjectId(99)));
        object.set_force_attack(true);
        object.set_target_location(Some(Vec3::new(1.0, 0.0, 2.0)));
        object.stop_attack();

        assert!(object.target.is_none());
        assert!(object.target_location.is_none());
        assert!(!object.force_attack);
        assert!(!object.status.attacking);
    }

    #[test]
    fn setting_target_location_clears_object_target() {
        let mut object = make_test_object();
        object.set_target(Some(ObjectId(77)));
        object.set_target_location(Some(Vec3::new(10.0, 0.0, 10.0)));

        assert!(object.target.is_none());
        assert!(object.target_location.is_some());
        assert!(object.status.attacking);
    }
}
