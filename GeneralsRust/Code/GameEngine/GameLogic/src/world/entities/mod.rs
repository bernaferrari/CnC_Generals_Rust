//! Entity storage and helpers mirroring the legacy object/thing system.
//!
//! The original engine routes almost everything through the global
//! `ObjectManager`.  Here we provide a modern, owned representation that still
//! uses familiar terminology (entity, template, owner) so porting code can stay
//! close to the C++ layout while benefiting from Rust's safety.

use crate::world::PlayerId;
use nalgebra::Point3;
use std::collections::HashMap;

/// Identifier assigned to entities/things in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId(u32);

impl EntityId {
    /// First valid entity identifier.
    pub const FIRST: EntityId = EntityId(1);

    /// Construct from a raw numeric id (shadow ID maps / diagnostics).
    pub fn from_raw(raw: u32) -> Self {
        EntityId(raw)
    }

    /// Raw numeric accessor.
    pub fn get(self) -> u32 {
        self.0
    }
}

/// Runtime description of a template. In the legacy engine this maps to
/// `ThingTemplate`.  We keep the fields intentionally small until the
/// higher-level systems are ported.
#[derive(Debug, Clone)]
pub struct TemplateRef {
    /// Stable name (matches C++ `ThingTemplate::GetName()`).
    pub name: String,
    /// Optional path to the definition file.
    pub source: Option<String>,
}

impl TemplateRef {
    /// Create a new template reference.
    pub fn new<S: Into<String>>(name: S) -> Self {
        Self {
            name: name.into(),
            source: None,
        }
    }
}

/// Minimal spatial information for an entity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// World-space position (X/Y/Z).
    pub position: Point3<f32>,
    /// Facing angle in radians.
    pub orientation: f32,
}

impl Transform {
    /// Create a new transform.
    pub fn new(position: [f32; 3], orientation: f32) -> Self {
        Self {
            position: Point3::from(position),
            orientation,
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Point3::new(0.0, 0.0, 0.0),
            orientation: 0.0,
        }
    }
}

/// Core runtime data for an entity.
#[derive(Debug, Clone)]
pub struct Entity {
    /// Unique identifier.
    pub id: EntityId,
    /// Template metadata.
    pub template: TemplateRef,
    /// Owning player (if any).
    pub owner: Option<PlayerId>,
    /// Spatial state.
    pub transform: Transform,
    /// Current hitpoints.
    pub health: f32,
    /// Attack/command target (shadow of host Object::target).
    pub attack_target: Option<EntityId>,
    /// Move destination (shadow of host movement.target_position).
    pub move_target: Option<[f32; 3]>,
    /// Host Object::max_health residual.
    pub max_health: f32,
    /// Host Object::selected residual (UI selection).
    pub selected: bool,
    /// Host Object::status.destroyed residual.
    pub destroyed: bool,
    /// Host Object::construction_percent residual (0..1).
    pub construction_percent: f32,
    /// Host Object::team residual as ordinal: 0 USA, 1 China, 2 GLA, 255 Neutral.
    pub team_ordinal: u8,
    /// Host Object::selection_radius residual.
    pub selection_radius: f32,
    /// Host Object::status.under_construction residual.
    pub under_construction: bool,
    /// Host Object::status.moving residual.
    pub moving: bool,
    /// Host Object::status.attacking residual.
    pub attacking: bool,
    /// Host Object::team_color residual (RGBA 0..1).
    pub team_color: [f32; 4],
    /// Host Object::power_provided residual.
    pub power_provided: i32,
    /// Host Object::power_consumed residual.
    pub power_consumed: i32,
    /// Host Object::object_type residual ordinal:
    /// 0 Infantry, 1 Vehicle, 2 Aircraft, 3 Building, 4 Supply, 5 Projectile, 6 Neutral.
    pub object_type_ordinal: u8,
    /// Host Object::max_transport residual (0 = heuristic default).
    pub max_transport: usize,
    /// Host Object::force_attack residual.
    pub force_attack: bool,
    /// Host Object::show_health_bar residual.
    pub show_health_bar: bool,
    /// Host Object::target_location residual (ground attack).
    pub target_location: Option<[f32; 3]>,
    /// Host Object::guard_position residual.
    pub guard_position: Option<[f32; 3]>,
    /// Host Object::guard_target residual as host object id (0 = none).
    pub guard_target_host: u32,
    /// Host Object::ai_state residual ordinal (see host_ai_state_ordinal).
    pub ai_state_ordinal: u8,
    /// Host Object::occupants.len residual (transport/garrison count).
    pub occupant_count: u16,
    /// Host Object::experience.current residual.
    pub experience_points: f32,
    /// Host Object::experience.level residual: 0 Rookie, 1 Veteran, 2 Elite, 3 Heroic.
    pub veterancy_ordinal: u8,
    /// Host Object::stored_resources.supplies residual.
    pub stored_supplies: u32,
    /// Host Object::status.stealthed residual.
    pub stealthed: bool,
    /// Host Object::status.detected residual.
    pub detected: bool,
    /// Host Object::status.using_ability residual.
    pub using_ability: bool,
    /// Host Object::status.airborne_target residual.
    pub airborne_target: bool,
    /// Host Object::status.disabled_underpowered residual.
    pub disabled_underpowered: bool,
    /// Host Object::status.disabled_unmanned residual.
    pub disabled_unmanned: bool,
    /// Host Object::status.disabled_hacked residual.
    pub disabled_hacked: bool,
    /// Host Object::building_data present residual.
    pub is_building: bool,
    /// Host BuildingType residual ordinal (0..12; 255 = not a building).
    pub building_type_ordinal: u8,
    /// Host BuildingData::production_queue.len residual.
    pub production_queue_len: u8,
    /// Head of production queue progress residual (0..1-ish).
    pub production_progress: f32,
    /// Head of production queue template name residual (empty if none).
    pub production_template: String,
    /// Host BuildingData::rally_point residual.
    pub rally_point: Option<[f32; 3]>,
    /// Host BuildingData::garrisoned_units.len residual.
    pub garrison_count: u16,
    /// Host BuildingData::max_garrison residual.
    pub max_garrison: u16,
    /// Host Object::weapon present residual.
    pub has_weapon: bool,
    /// Host Weapon::damage residual.
    pub weapon_damage: f32,
    /// Host Weapon::range residual.
    pub weapon_range: f32,
    /// Host Weapon::min_range residual.
    pub weapon_min_range: f32,
    /// Host Weapon::reload_time residual (seconds).
    pub weapon_reload_time: f32,
    /// Host Weapon::ammo residual (`u32::MAX` = unlimited/None).
    pub weapon_ammo: u32,
    /// Host Weapon::can_target_air residual.
    pub weapon_can_target_air: bool,
    /// Host Weapon::can_target_ground residual.
    pub weapon_can_target_ground: bool,
    /// Host Weapon::projectile_speed residual.
    pub weapon_projectile_speed: f32,
    /// Host secondary_weapon present residual.
    pub has_secondary_weapon: bool,
    /// Host Movement::max_speed residual.
    pub move_max_speed: f32,
    /// Host Movement::velocity residual.
    pub velocity: [f32; 3],
    /// Host Movement::path.len residual.
    pub path_len: u16,
    /// Host Movement::current_path_index residual.
    pub path_index: u16,
}

impl Entity {
    /// Convenience accessor for the template name.
    pub fn template_name(&self) -> &str {
        &self.template.name
    }
}

/// Store responsible for allocating and tracking entities.
#[derive(Debug, Default, Clone)]
pub struct EntityStore {
    next_id: u32,
    alive: HashMap<EntityId, Entity>,
}

impl EntityStore {
    /// Remove every entity and reset id allocation.
    pub fn clear(&mut self) {
        *self = Self::new();
    }

    /// Create a new store.
    pub fn new() -> Self {
        Self {
            next_id: EntityId::FIRST.get(),
            alive: HashMap::new(),
        }
    }

    /// Number of living entities.
    pub fn len(&self) -> usize {
        self.alive.len()
    }

    /// Returns true if no entities are alive.
    pub fn is_empty(&self) -> bool {
        self.alive.is_empty()
    }

    /// Iterate over entities.
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.alive.values()
    }

    /// Get a specific entity.
    pub fn get(&self, id: EntityId) -> Option<&Entity> {
        self.alive.get(&id)
    }

    /// Mutable accessor.
    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.alive.get_mut(&id)
    }

    /// Spawn a new entity using the provided template and initial state.
    pub fn spawn(
        &mut self,
        template: TemplateRef,
        owner: Option<PlayerId>,
        transform: Transform,
        health: f32,
    ) -> EntityId {
        let id = EntityId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1).max(EntityId::FIRST.get());

        let entity = Entity {
            id,
            template,
            owner,
            transform,
            health,
            attack_target: None,
            move_target: None,
            max_health: health.max(1.0),
            selected: false,
            destroyed: false,
            construction_percent: 1.0,
            team_ordinal: 255,
            selection_radius: 5.0,
            under_construction: false,
            moving: false,
            attacking: false,
            team_color: [1.0, 1.0, 1.0, 1.0],
            power_provided: 0,
            power_consumed: 0,
            object_type_ordinal: 6,
            max_transport: 0,
            force_attack: false,
            show_health_bar: true,
            target_location: None,
            guard_position: None,
            guard_target_host: 0,
            ai_state_ordinal: 0,
            occupant_count: 0,
            experience_points: 0.0,
            veterancy_ordinal: 0,
            stored_supplies: 0,
            stealthed: false,
            detected: false,
            using_ability: false,
            airborne_target: false,
            disabled_underpowered: false,
            disabled_unmanned: false,
            disabled_hacked: false,
            is_building: false,
            building_type_ordinal: 255,
            production_queue_len: 0,
            production_progress: 0.0,
            production_template: String::new(),
            rally_point: None,
            garrison_count: 0,
            max_garrison: 0,
            has_weapon: false,
            weapon_damage: 0.0,
            weapon_range: 0.0,
            weapon_min_range: 0.0,
            weapon_reload_time: 0.0,
            weapon_ammo: u32::MAX,
            weapon_can_target_air: false,
            weapon_can_target_ground: true,
            weapon_projectile_speed: 0.0,
            has_secondary_weapon: false,
            move_max_speed: 0.0,
            velocity: [0.0; 3],
            path_len: 0,
            path_index: 0,
        };

        self.alive.insert(id, entity);
        id
    }

    /// Remove an entity. Returns the removed entity if it was alive.
    pub fn remove(&mut self, id: EntityId) -> Option<Entity> {
        self.alive.remove(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_and_remove_entity() {
        let mut store = EntityStore::new();
        assert_eq!(store.len(), 0);

        let id = store.spawn(
            TemplateRef::new("GLAInfantryRebel"),
            Some(PlayerId::FIRST),
            Transform::new([10.0, 5.0, 0.0], 1.57),
            100.0,
        );

        let entity = store.get(id).expect("entity spawned");
        assert_eq!(entity.template_name(), "GLAInfantryRebel");
        assert_eq!(entity.owner, Some(PlayerId::FIRST));

        let removed = store.remove(id).expect("removed entity");
        assert_eq!(removed.id, id);
        assert!(store.is_empty());
    }
}
