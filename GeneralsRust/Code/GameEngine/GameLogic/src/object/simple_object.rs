//! SimpleObject class - Basic static objects
//!
//! SimpleObjects are basic game entities with minimal behavior, such as
//! props, decorations, resource nodes, crates, barriers, and other static elements.

use crate::common::ObjectID;
use crate::common::*;
use crate::damage::{DamageInfo, DamageType};
use crate::economy::ResourceType;
use crate::object::Object;
use crate::team::Team;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

/// Types of simple objects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimpleObjectType {
    Prop,         // Decorative object
    ResourceNode, // Resource deposit
    Crate,        // Supply crate
    Barrier,      // Impassable obstacle
    Destructible, // Destructible environment object
    Collectable,  // Can be picked up
    TechBuilding, // Neutral technology building
    Bridge,       // Traversable structure
    Civilian,     // Civilian structure/object
    Debris,       // Wreckage/debris
    Natural,      // Natural environment object (trees, rocks)
    Industrial,   // Industrial props (pipes, machinery)
}

/// Resource types that can be contained in simple objects
#[derive(Debug, Clone)]
pub struct ResourceContent {
    pub resource_type: ResourceType,
    pub amount: u32,
    pub max_amount: u32,
    pub regeneration_rate: Real, // For renewable resources
    pub is_infinite: bool,       // Infinite supply
}

/// Simple object-specific data and behavior
#[derive(Debug)]
pub struct SimpleObject {
    /// Base object functionality
    base_object: Arc<RwLock<Object>>,

    /// Object classification
    simple_object_type: SimpleObjectType,
    is_interactive: bool,
    is_destructible: bool,
    is_selectable: bool,

    /// Resource content (for resource nodes and crates)
    resource_contents: HashMap<ResourceType, ResourceContent>,
    total_value: u32,

    /// Collection properties
    can_be_collected: bool,
    collection_time: Real,        // Time required to collect/harvest
    requires_collector: bool,     // Needs specific unit type to collect
    collector_types: Vec<KindOf>, // What types of units can collect

    /// Destruction properties
    destruction_health: Real,
    destruction_effects: Vec<String>,
    destruction_sound: Option<String>,
    leaves_debris: bool,
    debris_template: Option<String>,

    /// Tech building properties (neutral buildings that can be captured)
    tech_building_bonus: Option<TechBuildingBonus>,
    can_be_captured: bool,
    capture_time: Real,
    capture_progress: Real,
    capturing_team: Option<Arc<RwLock<Team>>>,

    /// Bridge properties
    bridge_data: Option<BridgeData>,

    /// Civilian properties
    civilian_data: Option<CivilianData>,

    /// Visual and animation
    idle_animation: String,
    damage_animations: Vec<String>,
    destruction_animation: String,
    collection_animation: String,

    /// Physics properties
    blocks_movement: bool,
    blocks_projectiles: bool,
    collision_radius: Real,
    collision_height: Real,

    /// Interaction radius
    interaction_radius: Real, // How close units need to be to interact

    /// Spawn properties (for objects that spawn other objects)
    spawns_objects: bool,
    spawn_template: Option<String>,
    spawn_interval: Real,
    spawn_timer: Real,
    max_spawned_objects: u32,
    current_spawned_count: u32,

    /// Visibility and detection
    is_always_visible: bool,
    provides_vision: bool,
    vision_range: Real,

    /// Environmental effects
    ambient_sound: Option<String>,
    particle_effects: Vec<String>,
    light_emission: Option<LightData>,

    /// Status tracking
    is_depleted: bool, // For resources
    is_collected: bool, // For collectables
    last_interaction_time: Real,

    /// Special behaviors
    regenerates: bool,
    regeneration_rate: Real,
    regeneration_delay: Real, // Time before regeneration starts
}

/// Bonus provided by tech buildings
#[derive(Debug, Clone)]
pub struct TechBuildingBonus {
    pub bonus_type: TechBonusType,
    pub bonus_value: Real,
    pub affects_global: bool, // Affects all units vs. nearby units only
    pub bonus_radius: Real,   // Range for local bonuses
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechBonusType {
    HealthBonus,
    ArmorBonus,
    DamageBonus,
    SpeedBonus,
    VisionBonus,
    ResourceBonus,
    ProductionSpeedBonus,
    ResearchSpeedBonus,
    UnitCapacityBonus,
    SpecialAbility,
}

/// Bridge-specific data
#[derive(Debug, Clone)]
pub struct BridgeData {
    pub can_be_destroyed: bool,
    pub repair_cost: HashMap<ResourceType, u32>,
    pub repair_time: Real,
    pub is_functional: bool,
    pub blocks_water_units: bool,
    pub supports_weight: Real, // Maximum unit weight
}

/// Civilian object data
#[derive(Debug, Clone)]
pub struct CivilianData {
    pub population_value: u32, // How many civilians this represents
    pub morale_bonus: Real,    // Bonus to nearby friendly units
    pub intel_value: Real,     // Intelligence gathering bonus
    pub can_be_evacuated: bool,
}

/// Light emission data for objects that emit light
#[derive(Debug, Clone)]
pub struct LightData {
    pub intensity: Real,
    pub base_intensity: Real,
    pub radius: Real,
    pub color: Color,
    pub flicker_rate: Real,
    pub flicker_phase: Real,
    pub is_always_on: bool,
}

impl SimpleObject {
    pub fn base_object(&self) -> Arc<RwLock<Object>> {
        Arc::clone(&self.base_object)
    }

    /// Create a new SimpleObject
    pub fn new(
        base_object: Arc<RwLock<Object>>,
        thing_template: &dyn ThingTemplate,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let simple_object_type = Self::determine_simple_object_type(thing_template);
        let can_be_captured = thing_template.is_kind_of(KindOf::Capturable)
            && !thing_template.is_kind_of(KindOf::ImmuneToCapture);
        let capture_time = if can_be_captured { 1.0 } else { 0.0 };

        Ok(SimpleObject {
            base_object,
            simple_object_type,
            is_interactive: false,
            is_destructible: thing_template.is_kind_of(KindOf::Structure),
            is_selectable: thing_template.is_kind_of(KindOf::Selectable),

            resource_contents: HashMap::new(),
            total_value: 0,

            can_be_collected: false,
            collection_time: 0.0,
            requires_collector: false,
            collector_types: Vec::new(),

            destruction_health: 0.0,
            destruction_effects: Vec::new(),
            destruction_sound: None,
            leaves_debris: false,
            debris_template: None,

            tech_building_bonus: None,
            can_be_captured,
            capture_time,
            capture_progress: 0.0,
            capturing_team: None,

            bridge_data: None,
            civilian_data: None,

            idle_animation: String::new(),
            damage_animations: Vec::new(),
            destruction_animation: String::new(),
            collection_animation: String::new(),

            blocks_movement: false,
            blocks_projectiles: false,
            collision_radius: 0.0,
            collision_height: 0.0,

            interaction_radius: 0.0,

            spawns_objects: false,
            spawn_template: None,
            spawn_interval: 0.0,
            spawn_timer: 0.0,
            max_spawned_objects: 0,
            current_spawned_count: 0,

            is_always_visible: false,
            provides_vision: false,
            vision_range: thing_template.calc_vision_range(),

            ambient_sound: None,
            particle_effects: Vec::new(),
            light_emission: None,

            is_depleted: false,
            is_collected: false,
            last_interaction_time: 0.0,

            regenerates: false,
            regeneration_rate: 0.0,
            regeneration_delay: 0.0,
        })
    }

    /// Update simple object for one frame
    pub fn update(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.last_interaction_time += delta_time;

        // Update resource regeneration
        if self.regenerates && !self.is_depleted {
            self.update_resource_regeneration(delta_time)?;
        }

        // Update object spawning
        if self.spawns_objects {
            self.update_object_spawning(delta_time)?;
        }

        // Update capture progress
        if self.can_be_captured && self.capture_progress > 0.0 {
            self.update_capture_progress(delta_time)?;
        }

        // Update bridge state
        let health_pct = self.get_health_percentage();
        if let Some(bridge_data) = &mut self.bridge_data {
            Self::update_bridge_state(bridge_data, health_pct, delta_time)?;
        }

        // Update light effects
        if let Some(light_data) = &mut self.light_emission {
            Self::update_light_effects(light_data, delta_time)?;
        }

        Ok(())
    }

    /// Attempt to collect resources from this object
    pub fn collect_resources(
        &mut self,
        _collector: ObjectID,
        collection_rate: Real,
    ) -> Result<HashMap<ResourceType, u32>, Box<dyn std::error::Error + Send + Sync>> {
        if !self.can_be_collected || self.is_depleted {
            return Ok(HashMap::new());
        }

        let mut collected = HashMap::new();
        let mut check_depletion = false;

        // Check if collector is valid type
        // This would check the collector's KindOf flags against allowed types

        for (resource_type, content) in &mut self.resource_contents {
            if content.amount > 0 {
                let collection_amount =
                    (collection_rate * content.amount as Real).min(content.amount as Real) as u32;

                if collection_amount > 0 {
                    content.amount -= collection_amount;
                    collected.insert(*resource_type, collection_amount);

                    if content.amount == 0 && !content.is_infinite {
                        // Resource depleted
                        check_depletion = true;
                    }
                }
            }
        }

        if check_depletion {
            self.check_depletion();
        }

        if !collected.is_empty() {
            self.on_resource_collected(&collected)?;
        }

        Ok(collected)
    }

    /// Start capturing this object (for tech buildings)
    pub fn start_capture(
        &mut self,
        capturing_team: Arc<RwLock<Team>>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if !self.can_be_captured {
            return Ok(false);
        }

        self.capturing_team = Some(capturing_team);
        self.capture_progress = 0.0;

        Ok(true)
    }

    /// Stop capturing this object
    pub fn stop_capture(&mut self) {
        self.capturing_team = None;
        self.capture_progress = 0.0;
    }

    /// Complete capture of this object
    fn complete_capture(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(new_team) = self.capturing_team.take() {
            // Transfer ownership to capturing team
            if let Ok(mut obj_guard) = self.base_object.write() {
                obj_guard.set_team(Some(new_team))?;
            }

            // Apply tech building bonus to new team
            if let Some(bonus) = &self.tech_building_bonus {
                self.apply_tech_building_bonus(bonus)?;
            }

            self.capture_progress = 0.0;
        }

        Ok(())
    }

    /// Damage the simple object
    pub fn take_damage(
        &mut self,
        damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.is_destructible {
            return Ok(());
        }

        // Apply damage to base object
        let destroyed = if let Ok(mut obj_guard) = self.base_object.write() {
            obj_guard.attempt_damage(&mut damage_info.clone())?;
            obj_guard.get_health() <= 0.0
        } else {
            false
        };

        if destroyed {
            self.on_destroyed()?;
        }

        Ok(())
    }

    /// Check if object can be interacted with by the given unit
    pub fn can_interact_with(&self, _unit: ObjectID, unit_position: &Coord3D) -> bool {
        if !self.is_interactive {
            return false;
        }

        let object_position = self.get_position();
        let dx = object_position.x - unit_position.x;
        let dy = object_position.y - unit_position.y;
        let distance = (dx * dx + dy * dy).sqrt();

        if distance > self.interaction_radius {
            return false;
        }

        // Check specific interaction conditions based on object type
        match self.simple_object_type {
            SimpleObjectType::ResourceNode => !self.is_depleted,
            SimpleObjectType::Crate => !self.is_collected,
            SimpleObjectType::TechBuilding => self.can_be_captured,
            SimpleObjectType::Collectable => !self.is_collected,
            _ => true,
        }
    }

    /// Get the current position of the object
    pub fn get_position(&self) -> Coord3D {
        if let Ok(obj_guard) = self.base_object.read() {
            *obj_guard.get_position()
        } else {
            Coord3D::new(0.0, 0.0, 0.0)
        }
    }

    /// Get remaining resources
    pub fn get_remaining_resources(&self) -> HashMap<ResourceType, u32> {
        self.resource_contents
            .iter()
            .map(|(rt, content)| (*rt, content.amount))
            .collect()
    }

    /// Check if object is depleted of resources
    pub fn is_resource_depleted(&self) -> bool {
        self.is_depleted
            || self
                .resource_contents
                .values()
                .all(|c| c.amount == 0 && !c.is_infinite)
    }

    /// Get tech building bonus if applicable
    pub fn get_tech_building_bonus(&self) -> Option<&TechBuildingBonus> {
        self.tech_building_bonus.as_ref()
    }

    /// Check if this object blocks movement
    pub fn blocks_unit_movement(&self) -> bool {
        self.blocks_movement && !self.is_destroyed()
    }

    /// Check if this object blocks projectiles
    pub fn blocks_projectile_movement(&self) -> bool {
        self.blocks_projectiles && !self.is_destroyed()
    }

    /// Check if object is destroyed
    pub fn is_destroyed(&self) -> bool {
        if let Ok(obj_guard) = self.base_object.read() {
            obj_guard.is_destroyed()
        } else {
            true
        }
    }

    // Private helper methods

    fn determine_simple_object_type(thing_template: &dyn ThingTemplate) -> SimpleObjectType {
        if thing_template.is_kind_of(KindOf::ResourceNode) {
            SimpleObjectType::ResourceNode
        } else if thing_template.is_kind_of(KindOf::Crate) {
            SimpleObjectType::Crate
        } else if thing_template.is_kind_of(KindOf::TechBuilding) {
            SimpleObjectType::TechBuilding
        } else if thing_template.is_kind_of(KindOf::Bridge) {
            SimpleObjectType::Bridge
        } else if thing_template.is_kind_of(KindOf::Barrier) {
            SimpleObjectType::Barrier
        } else if thing_template.is_kind_of(KindOf::Civilian) {
            SimpleObjectType::Civilian
        } else if thing_template.is_kind_of(KindOf::Destructible) {
            SimpleObjectType::Destructible
        } else {
            SimpleObjectType::Prop
        }
    }

    fn update_resource_regeneration(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.last_interaction_time < self.regeneration_delay {
            return Ok(()); // Still in delay period
        }

        for (_, content) in &mut self.resource_contents {
            if content.regeneration_rate > 0.0 && content.amount < content.max_amount {
                let regen_amount = content.regeneration_rate * delta_time;
                content.amount =
                    (content.amount as Real + regen_amount).min(content.max_amount as Real) as u32;

                if content.amount > 0 {
                    self.is_depleted = false;
                }
            }
        }

        Ok(())
    }

    fn update_object_spawning(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.current_spawned_count >= self.max_spawned_objects {
            return Ok(());
        }

        self.spawn_timer += delta_time;
        if self.spawn_timer >= self.spawn_interval {
            self.spawn_timer = 0.0;
            self.spawn_object()?;
        }

        Ok(())
    }

    fn update_capture_progress(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(_) = &self.capturing_team {
            self.capture_progress += delta_time / self.capture_time;

            if self.capture_progress >= 1.0 {
                self.complete_capture()?;
            }
        } else {
            // Decay capture progress if no one is capturing
            self.capture_progress -= delta_time * 0.5; // Decay rate
            if self.capture_progress < 0.0 {
                self.capture_progress = 0.0;
            }
        }

        Ok(())
    }

    fn update_bridge_state(
        bridge_data: &mut BridgeData,
        health_percentage: Real,
        _delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update bridge functionality based on damage
        bridge_data.is_functional = health_percentage > 0.25; // Bridge fails at 25% health

        Ok(())
    }

    fn update_light_effects(
        light_data: &mut LightData,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if light_data.flicker_rate > 0.0 {
            // Update phase
            light_data.flicker_phase += delta_time * light_data.flicker_rate;

            // Calculate flicker factor (sine wave +/- 20%)
            let flicker = (light_data.flicker_phase).sin() * 0.2;

            // Apply to base intensity
            light_data.intensity = light_data.base_intensity * (1.0 + flicker);

            // Ensure intensity doesn't go negative
            if light_data.intensity < 0.0 {
                light_data.intensity = 0.0;
            }
        }

        Ok(())
    }

    fn check_depletion(&mut self) {
        self.is_depleted = self
            .resource_contents
            .values()
            .all(|c| c.amount == 0 && !c.is_infinite);
    }

    fn on_resource_collected(
        &mut self,
        _collected: &HashMap<ResourceType, u32>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Play collection effects
        if !self.collection_animation.is_empty() {
            self.play_animation(&self.collection_animation);
        }

        Ok(())
    }

    fn on_destroyed(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Play destruction effects
        if !self.destruction_animation.is_empty() {
            self.play_animation(&self.destruction_animation);
        }

        for effect in &self.destruction_effects {
            self.play_effect(effect);
        }

        if let Some(sound) = &self.destruction_sound {
            self.play_sound(sound);
        }

        // Spawn debris if configured
        if self.leaves_debris {
            if let Some(debris_template) = &self.debris_template {
                self.spawn_debris(debris_template)?;
            }
        }

        Ok(())
    }

    fn apply_tech_building_bonus(
        &self,
        _bonus: &TechBuildingBonus,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Apply the tech building bonus to the capturing team
        // This would involve updating the team's global bonuses or local area effects
        Ok(())
    }

    fn spawn_object(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(_template) = &self.spawn_template {
            // Create new object from template
            // This would use the object factory
            self.current_spawned_count += 1;
        }

        Ok(())
    }

    fn spawn_debris(
        &self,
        _debris_template: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Spawn debris object at current location
        Ok(())
    }

    fn get_health_percentage(&self) -> Real {
        if let Ok(obj_guard) = self.base_object.read() {
            let current = obj_guard.get_health();
            let max = obj_guard.get_max_health();
            if max > 0.0 {
                current / max
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    fn play_animation(&self, animation: &str) {
        if animation.is_empty() {
            return;
        }
        if let Ok(base_guard) = self.base_object.read() {
            if let Some(drawable) = base_guard.get_drawable() {
                if let Ok(mut drawable_guard) = drawable.write() {
                    let _ = drawable_guard.play_animation(animation, false, 0.0);
                }
            }
        }
    }

    fn play_effect(&self, effect: &str) {
        if effect.is_empty() {
            return;
        }
        let pos = self
            .base_object
            .read()
            .map(|guard| *guard.get_position())
            .unwrap_or_else(|_| Coord3D::origin());
        if let Some(fx) = crate::helpers::TheFXList::get() {
            fx.do_fx_at_position(effect, &pos);
        }
    }

    fn play_sound(&self, sound: &str) {
        if sound.is_empty() {
            return;
        }
        if let Ok(base_guard) = self.base_object.read() {
            let pos = *base_guard.get_position();
            if let Some(audio) = crate::helpers::TheAudio::get() {
                let mut event = crate::common::audio::AudioEventRts::new(sound);
                event.set_object_id(base_guard.get_id() as u32);
                event.set_position(&(pos.x, pos.y, pos.z));
                let _ = audio.add_audio_event(&event);
            }
        }
    }
}

/// Extension trait for Object to provide SimpleObject-specific functionality
pub trait SimpleObjectExt {
    /// Get simple object-specific data if this object is a simple object
    fn as_simple_object(&self) -> Option<&SimpleObject>;
    fn as_simple_object_mut(&mut self) -> Option<&mut SimpleObject>;
}

// This would need to be implemented for the actual Object type
// impl SimpleObjectExt for Object {
//     fn as_simple_object(&self) -> Option<&SimpleObject> {
//         // Implementation would check if this object is actually a simple object
//         None
//     }
//
//     fn as_simple_object_mut(&mut self) -> Option<&mut SimpleObject> {
//         // Implementation would check if this object is actually a simple object
//         None
//     }
// }
