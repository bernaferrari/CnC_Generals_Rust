use super::*;
use serde::{Deserialize, Serialize};

/// Unit-specific data and behaviors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitData {
    pub unit_type: UnitType,
    pub squad_size: usize,
    pub formation_offset: Vec2,
    pub resource_gatherer: Option<ResourceGatherer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitType {
    Infantry,
    Vehicle,
    Aircraft,
}

impl UnitData {
    pub fn new_infantry() -> Self {
        Self {
            unit_type: UnitType::Infantry,
            squad_size: 1,
            formation_offset: Vec2::ZERO,
            resource_gatherer: None,
        }
    }

    pub fn new_vehicle() -> Self {
        Self {
            unit_type: UnitType::Vehicle,
            squad_size: 1,
            formation_offset: Vec2::ZERO,
            resource_gatherer: None,
        }
    }

    pub fn new_worker() -> Self {
        Self {
            unit_type: UnitType::Infantry,
            squad_size: 1,
            formation_offset: Vec2::ZERO,
            resource_gatherer: Some(ResourceGatherer::default()),
        }
    }

    pub fn can_gather_resources(&self) -> bool {
        self.resource_gatherer.is_some()
    }
}

/// Unit factory functions
pub fn create_unit_templates() -> HashMap<String, ThingTemplate> {
    let mut templates = HashMap::new();

    // GLA Units
    let mut gla_soldier = ThingTemplate::new("GLA_Soldier");
    gla_soldier
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(50.0)
        .set_cost(60, 0)
        .set_model("githrpf") // GLA Infantry model
        .set_primary_weapon_name(super::weapon_bootstrap::GLA_REBEL_PRIMARY_WEAPON)
        .set_locomotor_name(super::locomotor_bootstrap::BASIC_HUMAN_LOCOMOTOR);
    templates.insert("GLA_Soldier".to_string(), gla_soldier);

    let mut gla_worker = ThingTemplate::new("GLA_Worker");
    gla_worker
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(40.0)
        .set_cost(200, 0)
        .set_model("giworker"); // GLA Worker model
    templates.insert("GLA_Worker".to_string(), gla_worker);

    let mut gla_technical = ThingTemplate::new("GLA_Technical");
    gla_technical
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_cost(400, 0)
        .set_model("gvtchncl") // GLA Technical vehicle
        .set_locomotor_name(super::locomotor_bootstrap::TECHNICAL_LOCOMOTOR);
    templates.insert("GLA_Technical".to_string(), gla_technical);

    let mut gla_scorpion = ThingTemplate::new("GLA_Scorpion");
    gla_scorpion
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0)
        .set_cost(800, 0)
        .set_model("gvscorpion") // GLA Scorpion tank
        .set_locomotor_name(super::locomotor_bootstrap::SCORPION_LOCOMOTOR);
    templates.insert("GLA_Scorpion".to_string(), gla_scorpion);

    // USA Units
    let mut usa_ranger = ThingTemplate::new("USA_Ranger");
    usa_ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(60.0)
        .set_cost(80, 0)
        .set_model("airanger") // USA Ranger infantry
        .set_primary_weapon_name(super::weapon_bootstrap::RANGER_PRIMARY_WEAPON)
        .set_secondary_weapon_name(super::weapon_bootstrap::RANGER_SECONDARY_WEAPON)
        .set_locomotor_name(super::locomotor_bootstrap::BASIC_HUMAN_LOCOMOTOR);
    templates.insert("USA_Ranger".to_string(), usa_ranger);

    let mut usa_dozer = ThingTemplate::new("USA_Dozer");
    usa_dozer
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0)
        .set_cost(1000, 0)
        .set_model("avdozer"); // USA Construction Dozer
    templates.insert("USA_Dozer".to_string(), usa_dozer);

    let mut usa_humvee = ThingTemplate::new("USA_Humvee");
    usa_humvee
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(250.0)
        .set_cost(600, 0)
        .set_model("avhummer") // USA Humvee
        .set_primary_weapon_name(super::weapon_bootstrap::HUMVEE_PRIMARY_WEAPON)
        .set_secondary_weapon_name(super::weapon_bootstrap::HUMVEE_SECONDARY_WEAPON)
        .set_locomotor_name(super::locomotor_bootstrap::HUMVEE_LOCOMOTOR);
    templates.insert("USA_Humvee".to_string(), usa_humvee);

    let mut usa_crusader = ThingTemplate::new("USA_Crusader");
    usa_crusader
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(600.0)
        .set_cost(1200, 0)
        .set_model("avcrusader") // USA Crusader tank
        .set_locomotor_name(super::locomotor_bootstrap::CRUSADER_LOCOMOTOR);
    templates.insert("USA_Crusader".to_string(), usa_crusader);

    // China Units
    let mut china_soldier = ThingTemplate::new("China_Soldier");
    china_soldier
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(55.0)
        .set_cost(75, 0)
        .set_model("cirifle") // China Red Guard infantry
        .set_primary_weapon_name(super::weapon_bootstrap::REDGUARD_PRIMARY_WEAPON)
        .set_locomotor_name(super::locomotor_bootstrap::REDGUARD_LOCOMOTOR);
    templates.insert("China_Soldier".to_string(), china_soldier);

    let mut china_dozer = ThingTemplate::new("China_Dozer");
    china_dozer
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(350.0)
        .set_cost(1000, 0)
        .set_model("cvdozer"); // China Construction Dozer
    templates.insert("China_Dozer".to_string(), china_dozer);

    let mut china_battletank = ThingTemplate::new("China_BattleTank");
    china_battletank
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0)
        .set_cost(1000, 0)
        .set_model("cvbattlemaster") // China Battle Master tank
        .set_locomotor_name(super::locomotor_bootstrap::BATTLE_MASTER_LOCOMOTOR);
    templates.insert("China_BattleTank".to_string(), china_battletank);

    templates
}

/// Unit behavior system
pub struct UnitBehavior;

impl UnitBehavior {
    /// Update gathering behavior for worker units
    pub fn update_resource_gathering(
        object_id: ObjectId,
        objects: &mut HashMap<ObjectId, Object>,
        _resource_manager: &mut ResourceManager,
        dt: f32,
    ) -> bool {
        let mut needs_return = false;
        let mut gathered_amount = 0u32;
        let _source_id: Option<ObjectId> = None;

        // Get gathering info from object
        if let Some(obj) = objects.get_mut(&object_id) {
            if let Some(_gatherer) = &mut obj.movement.path.first().copied() {
                // Simplified gathering logic - would need proper resource gatherer component
                if obj.ai_state == AIState::Gathering {
                    // Simulate gathering
                    gathered_amount = (100.0 * dt) as u32; // 100 resources per second
                    if gathered_amount > 0 {
                        needs_return = true;
                    }
                }
            }
        }

        // Process gathered resources
        if gathered_amount > 0 && needs_return {
            // Find nearest supply center to deposit
            if let Some(obj) = objects.get(&object_id) {
                let position = obj.get_position();
                let team = obj.team;

                // Find nearest supply center for this team
                for (_, other_obj) in objects.iter() {
                    if other_obj.team == team
                        && other_obj.is_kind_of(KindOf::SupplyCenter)
                        && other_obj.is_constructed()
                        && other_obj.is_alive()
                    {
                        let distance = position.distance(other_obj.get_position());
                        if distance < 50.0 {
                            // Close enough to deposit
                            // Add resources to player
                            return true; // Indicate successful deposit
                        }
                    }
                }
            }
        }

        false
    }

    /// Update formation movement for squad units
    pub fn update_formation_movement(
        leader_id: ObjectId,
        squad_members: &[ObjectId],
        objects: &mut HashMap<ObjectId, Object>,
        _dt: f32,
    ) {
        if let Some(leader) = objects.get(&leader_id) {
            let leader_pos = leader.get_position();
            let leader_angle = leader.get_orientation();

            // Update squad members to maintain formation
            for (i, &member_id) in squad_members.iter().enumerate() {
                if member_id == leader_id {
                    continue; // Skip leader
                }

                if let Some(member) = objects.get_mut(&member_id) {
                    // Calculate formation position
                    let offset_x = (i as f32 - 1.0) * 10.0; // Side-by-side formation
                    let offset_z = -20.0; // Behind leader

                    let cos_angle = leader_angle.cos();
                    let sin_angle = leader_angle.sin();

                    let formation_pos = Vec3::new(
                        leader_pos.x + offset_x * cos_angle - offset_z * sin_angle,
                        leader_pos.y,
                        leader_pos.z + offset_x * sin_angle + offset_z * cos_angle,
                    );

                    // Move to formation position if not already there
                    let distance_to_formation = member.get_position().distance(formation_pos);
                    if distance_to_formation > 5.0 {
                        member.move_to(formation_pos);
                    }
                }
            }
        }
    }

    /// Update unit AI based on current state and situation
    pub fn update_unit_ai(object_id: ObjectId, objects: &mut HashMap<ObjectId, Object>, dt: f32) {
        let ai_state = {
            if let Some(obj) = objects.get(&object_id) {
                obj.ai_state.clone()
            } else {
                return;
            }
        };

        match ai_state {
            AIState::Idle => {
                // Look for enemies in sight range
                Self::find_and_attack_enemies(object_id, objects);
            }
            AIState::Patrolling => {
                // Implement patrol behavior
                Self::update_patrol_behavior(object_id, objects, dt);
            }
            AIState::GuardingArea | AIState::GuardingObject => {
                // Implement guard behavior
                Self::update_guard_behavior(object_id, objects, dt);
            }
            _ => {} // Other states handled elsewhere
        }
    }

    fn find_and_attack_enemies(object_id: ObjectId, objects: &mut HashMap<ObjectId, Object>) {
        let (position, team, sight_range) = {
            if let Some(obj) = objects.get(&object_id) {
                if !obj.can_attack() {
                    return;
                }
                (obj.get_position(), obj.team, obj.get_template().sight_range)
            } else {
                return;
            }
        };

        // Find nearest enemy in sight range
        let mut nearest_enemy = None;
        let mut nearest_distance = sight_range;

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
            if let Some(obj) = objects.get_mut(&object_id) {
                obj.attack_target(enemy_id);
            }
        }
    }

    fn update_patrol_behavior(
        object_id: ObjectId,
        objects: &mut HashMap<ObjectId, Object>,
        _dt: f32,
    ) {
        // Simplified patrol - move between waypoints
        // In a full implementation, this would cycle through patrol points
        if let Some(obj) = objects.get_mut(&object_id) {
            if obj.movement.target_position.is_none() {
                // Set a random patrol destination
                let current_pos = obj.get_position();
                let patrol_radius = 100.0;
                let random_angle = fastrand::f32() * 2.0 * std::f32::consts::PI;
                let patrol_pos = Vec3::new(
                    current_pos.x + patrol_radius * random_angle.cos(),
                    current_pos.y,
                    current_pos.z + patrol_radius * random_angle.sin(),
                );
                obj.move_to(patrol_pos);
            }
        }
    }

    fn update_guard_behavior(
        object_id: ObjectId,
        objects: &mut HashMap<ObjectId, Object>,
        _dt: f32,
    ) {
        // Guard behavior - stay near guard position and attack enemies
        Self::find_and_attack_enemies(object_id, objects);

        // Return to guard position if moved too far (simplified)
        if let Some(obj) = objects.get_mut(&object_id) {
            if obj.movement.target_position.is_none() && obj.target.is_none() {
                // Would return to original guard position in full implementation
                obj.ai_state = AIState::Idle;
            }
        }
    }
}
