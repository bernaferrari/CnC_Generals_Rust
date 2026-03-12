//! # FXList Integration with Particle System
//!
//! Bridges the gap between the FXList system (from C++ GameClient/FXList.cpp)
//! and the modern Rust particle system. Allows FXLists to spawn particle systems
//! and coordinate complex visual effects.
//!
//! This matches the C++ behavior where FXLists can contain ParticleSystemFXNuggets
//! that create and manage particle systems as part of larger effect sequences.

use super::particle_manager::*;
use super::particle_presets;
use nalgebra::{Matrix3, Point3, Rotation3, Vector3};
use std::collections::HashMap;
use std::sync::Arc;

/// FX nugget that spawns a particle system
/// Matches C++ ParticleSystemFXNugget from FXList.cpp:481-658
pub struct ParticleSystemFXNugget {
    /// Particle system template name
    pub template_name: String,

    /// Number of systems to spawn
    pub count: i32,

    /// Offset from primary position
    pub offset: Vector3<f32>,

    /// Random radius distribution
    pub radius: GameClientRandomVariable,

    /// Random height variation
    pub height: GameClientRandomVariable,

    /// Delay before spawning (frames)
    pub delay: GameClientRandomVariable,

    /// Rotation around axes
    pub rotate_x: f32,
    pub rotate_y: f32,
    pub rotate_z: f32,

    /// Orientation flags
    pub orient_to_object: bool,
    pub ricochet: bool,
    pub attach_to_object: bool,
    pub create_at_ground_height: bool,
    pub use_callers_radius: bool,
}

impl Default for ParticleSystemFXNugget {
    fn default() -> Self {
        Self {
            template_name: String::new(),
            count: 1,
            offset: Vector3::zeros(),
            radius: GameClientRandomVariable::new(0.0, 0.0),
            height: GameClientRandomVariable::new(0.0, 0.0),
            delay: GameClientRandomVariable::new(-1.0, -1.0),
            rotate_x: 0.0,
            rotate_y: 0.0,
            rotate_z: 0.0,
            orient_to_object: false,
            ricochet: false,
            attach_to_object: false,
            create_at_ground_height: false,
            use_callers_radius: false,
        }
    }
}

impl ParticleSystemFXNugget {
    /// Create a new particle system FX nugget
    pub fn new(template_name: String) -> Self {
        Self {
            template_name,
            ..Default::default()
        }
    }

    /// Execute the FX at a position
    /// Matches C++ ParticleSystemFXNugget::doFXPos
    pub fn do_fx_pos(
        &self,
        primary: Point3<f32>,
        primary_mtx: Option<&Matrix3<f32>>,
        override_radius: f32,
        manager: &mut ParticleSystemManager,
    ) -> Vec<ParticleSystemId> {
        self.really_do_fx(primary, primary_mtx, None, override_radius, manager)
    }

    /// Execute the FX attached to an object
    /// Matches C++ ParticleSystemFXNugget::doFXObj
    pub fn do_fx_obj(
        &self,
        primary: Point3<f32>,
        primary_mtx: Option<&Matrix3<f32>>,
        object_id: Option<ObjectId>,
        manager: &mut ParticleSystemManager,
    ) -> Vec<ParticleSystemId> {
        self.really_do_fx(primary, primary_mtx, object_id, 0.0, manager)
    }

    /// Actually create the particle systems
    /// Matches C++ ParticleSystemFXNugget::reallyDoFX (lines 570-641)
    fn really_do_fx(
        &self,
        primary: Point3<f32>,
        mtx: Option<&Matrix3<f32>>,
        object_id: Option<ObjectId>,
        override_radius: f32,
        manager: &mut ParticleSystemManager,
    ) -> Vec<ParticleSystemId> {
        let mut created_systems = Vec::new();

        // Find or create template
        let template =
            if let Some(preset) = particle_presets::get_preset_by_name(&self.template_name) {
                preset
            } else if let Some(tmpl) = manager.find_template(&self.template_name) {
                tmpl
            } else {
                // Template not found, return empty
                return created_systems;
            };

        // Apply offset with matrix transformation
        let mut offset = self.offset;
        if let Some(matrix) = mtx {
            offset = matrix * offset;
        }

        // Create multiple systems based on count
        for _ in 0..self.count {
            let radius = self.radius.sample();
            let height_offset = self.height.sample();
            let angle = rand::random::<f32>() * 2.0 * std::f32::consts::PI;

            let mut spawn_pos = Point3::new(
                primary.x + offset.x + radius * angle.cos(),
                primary.y + offset.y + radius * angle.sin(),
                primary.z + offset.z + height_offset,
            );

            // Ground height adjustment
            if self.create_at_ground_height {
                // C++ parity intent: clamp to terrain. Until terrain query is threaded through
                // this FX path, keep the caller-provided ground plane instead of forcing 0.0.
                spawn_pos.z = primary.z + offset.z + height_offset;
            }

            // Create particle system
            let result = if let Some(obj_id) = object_id {
                if self.attach_to_object {
                    manager.create_attached_particle_system(&template, obj_id, true)
                } else {
                    manager.create_particle_system(&template, true)
                }
            } else {
                manager.create_particle_system(&template, true)
            };

            if let Ok(system_id) = result {
                // Set system position
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.set_position(spawn_pos);

                    // Apply rotation if specified
                    if self.rotate_x != 0.0 || self.rotate_y != 0.0 || self.rotate_z != 0.0 {
                        let rotation_matrix = Rotation3::from_euler_angles(
                            self.rotate_x,
                            self.rotate_y,
                            self.rotate_z,
                        )
                        .into_inner();
                        system.set_local_transform(rotation_matrix);
                    }

                    // Apply caller's radius if requested
                    if override_radius > 0.0 && self.use_callers_radius {
                        match system.get_emission_volume_type() {
                            EmissionVolumeType::Sphere => {
                                system.set_emission_volume_sphere_radius(override_radius);
                            }
                            EmissionVolumeType::Cylinder => {
                                system.set_emission_volume_cylinder_radius(override_radius);
                            }
                            _ => {}
                        }
                    }

                    // Start the system
                    system.start();
                }

                created_systems.push(system_id);
            }
        }

        created_systems
    }
}

/// FXList bridge for particle effects
/// Allows FXLists to create and manage particle systems
pub struct FXListParticleBridge {
    /// Registered FX nuggets by name
    nuggets: HashMap<String, ParticleSystemFXNugget>,

    /// Active particle systems spawned by FXLists
    active_systems: Vec<ParticleSystemId>,
}

impl FXListParticleBridge {
    pub fn new() -> Self {
        Self {
            nuggets: HashMap::new(),
            active_systems: Vec::new(),
        }
    }

    /// Register a particle system FX nugget
    pub fn register_nugget(&mut self, name: String, nugget: ParticleSystemFXNugget) {
        self.nuggets.insert(name, nugget);
    }

    /// Execute FX by name
    pub fn execute_fx(
        &mut self,
        name: &str,
        position: Point3<f32>,
        transform: Option<&Matrix3<f32>>,
        manager: &mut ParticleSystemManager,
    ) -> Vec<ParticleSystemId> {
        if let Some(nugget) = self.nuggets.get(name) {
            let systems = nugget.do_fx_pos(position, transform, 0.0, manager);
            self.active_systems.extend(systems.clone());
            systems
        } else {
            Vec::new()
        }
    }

    /// Execute FX with object attachment
    pub fn execute_fx_on_object(
        &mut self,
        name: &str,
        position: Point3<f32>,
        transform: Option<&Matrix3<f32>>,
        object_id: ObjectId,
        manager: &mut ParticleSystemManager,
    ) -> Vec<ParticleSystemId> {
        if let Some(nugget) = self.nuggets.get(name) {
            let systems = nugget.do_fx_obj(position, transform, Some(object_id), manager);
            self.active_systems.extend(systems.clone());
            systems
        } else {
            Vec::new()
        }
    }

    /// Clean up finished systems
    pub fn cleanup_finished_systems(&mut self, manager: &mut ParticleSystemManager) {
        self.active_systems.retain(|&system_id| {
            if let Some(system) = manager.find_particle_system(system_id) {
                !system.is_destroyed() && system.particle_count() > 0
            } else {
                false
            }
        });
    }

    /// Get active system count
    pub fn active_system_count(&self) -> usize {
        self.active_systems.len()
    }
}

impl Default for FXListParticleBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions for common FX operations
pub mod helpers {
    use super::*;

    /// Create explosion FX at position
    pub fn create_explosion_at(
        position: Point3<f32>,
        explosion_type: &str,
        manager: &mut ParticleSystemManager,
    ) -> Option<ParticleSystemId> {
        let template = particle_presets::get_preset_by_name(explosion_type)?;
        let system_id = manager.create_particle_system(&template, true).ok()?;

        if let Some(system) = manager.find_particle_system_mut(system_id) {
            system.set_position(position);
            system.trigger(); // Immediate burst
        }

        Some(system_id)
    }

    /// Create weapon fire FX (muzzle flash + smoke)
    pub fn create_weapon_fire_fx(
        muzzle_position: Point3<f32>,
        muzzle_direction: Vector3<f32>,
        manager: &mut ParticleSystemManager,
    ) -> Vec<ParticleSystemId> {
        let mut systems = Vec::new();

        // Muzzle flash
        if let Some(flash_template) = particle_presets::get_preset_by_name("MuzzleFlash") {
            if let Ok(flash_id) = manager.create_particle_system(&flash_template, false) {
                if let Some(flash_system) = manager.find_particle_system_mut(flash_id) {
                    flash_system.set_position(muzzle_position);
                    flash_system.trigger();
                    systems.push(flash_id);
                }
            }
        }

        // Shell casing smoke
        if let Some(smoke_template) = particle_presets::get_preset_by_name("ShellCasingSmoke") {
            if let Ok(smoke_id) = manager.create_particle_system(&smoke_template, false) {
                if let Some(smoke_system) = manager.find_particle_system_mut(smoke_id) {
                    // Offset slightly to side for ejection
                    let side_offset = Vector3::new(-muzzle_direction.y, muzzle_direction.x, 0.0)
                        .normalize()
                        * 2.0;
                    smoke_system.set_position(muzzle_position + side_offset);
                    smoke_system.trigger();
                    systems.push(smoke_id);
                }
            }
        }

        systems
    }

    /// Create building destruction FX
    pub fn create_building_destruction_fx(
        building_center: Point3<f32>,
        building_size: f32,
        manager: &mut ParticleSystemManager,
    ) -> Vec<ParticleSystemId> {
        let mut systems = Vec::new();

        // Main explosion
        if let Some(explosion) = create_explosion_at(building_center, "LargeExplosion", manager) {
            systems.push(explosion);
        }

        // Collapse dust
        if let Some(dust_template) = particle_presets::get_preset_by_name("BuildingCollapseDust") {
            if let Ok(dust_id) = manager.create_particle_system(&dust_template, false) {
                if let Some(dust_system) = manager.find_particle_system_mut(dust_id) {
                    dust_system.set_position(building_center);
                    dust_system.trigger();
                    systems.push(dust_id);
                }
            }
        }

        // Debris
        if let Some(debris_template) = particle_presets::get_preset_by_name("BuildingDebris") {
            if let Ok(debris_id) = manager.create_particle_system(&debris_template, false) {
                if let Some(debris_system) = manager.find_particle_system_mut(debris_id) {
                    debris_system.set_position(building_center);
                    debris_system.trigger();
                    systems.push(debris_id);
                }
            }
        }

        systems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_particle_system_fx_nugget() {
        let nugget = ParticleSystemFXNugget::new("SmallExplosion".to_string());
        assert_eq!(nugget.template_name, "SmallExplosion");
        assert_eq!(nugget.count, 1);
    }

    #[test]
    fn test_fxlist_bridge() {
        let mut bridge = FXListParticleBridge::new();
        let nugget = ParticleSystemFXNugget::new("MuzzleFlash".to_string());

        bridge.register_nugget("TestFX".to_string(), nugget);
        assert_eq!(bridge.active_system_count(), 0);
    }

    #[test]
    fn test_explosion_helper() {
        let mut manager = ParticleSystemManager::new();
        let position = Point3::new(100.0, 200.0, 0.0);

        let system_id = helpers::create_explosion_at(position, "SmallExplosion", &mut manager);
        assert!(system_id.is_some());

        if let Some(id) = system_id {
            let system = manager.find_particle_system(id);
            assert!(system.is_some());
        }
    }

    #[test]
    fn test_weapon_fire_helper() {
        let mut manager = ParticleSystemManager::new();
        let muzzle_pos = Point3::new(10.0, 20.0, 5.0);
        let muzzle_dir = Vector3::new(1.0, 0.0, 0.0);

        let systems = helpers::create_weapon_fire_fx(muzzle_pos, muzzle_dir, &mut manager);
        assert!(!systems.is_empty());
    }

    #[test]
    fn test_building_destruction_helper() {
        let mut manager = ParticleSystemManager::new();
        let building_center = Point3::new(50.0, 50.0, 0.0);

        let systems = helpers::create_building_destruction_fx(building_center, 20.0, &mut manager);
        assert!(!systems.is_empty());
    }
}
