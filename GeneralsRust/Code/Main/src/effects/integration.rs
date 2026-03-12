use glam::{Mat4, Vec3};
use log::{debug, info};
use std::sync::{Arc, Mutex};
use ww3d_engine::FrameTiming;

use crate::assets::archive::ArchiveFileSystem;
use crate::effects::animation_system::AnimationManager;
use crate::effects::audio_integration::EnhancedAudioManager;
use crate::effects::lighting_system::DynamicLighting;
use crate::effects::particle_system::ParticleSystemManager;
use crate::effects::performance::{AdaptivePerformanceManager, QualityLevel};
use crate::effects::visual_effects::VisualEffectsManager;
use crate::game_logic::ObjectId;

/// Master effects coordinator that manages all visual and audio effects
pub struct EffectsIntegration {
    // Core systems
    particle_manager: Arc<Mutex<ParticleSystemManager>>,
    animation_manager: Arc<Mutex<AnimationManager>>,
    audio_manager: EnhancedAudioManager,
    visual_effects: VisualEffectsManager,
    lighting_system: DynamicLighting,
    performance_manager: AdaptivePerformanceManager,

    // State
    current_time: f32,
    camera_position: Vec3,
    camera_forward: Vec3,
    camera_up: Vec3,
    is_initialized: bool,
}

impl EffectsIntegration {
    /// Create a new effects integration system
    pub async fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        quality_level: QualityLevel,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Initializing C&C Generals Effects Integration System");

        // Initialize particle system
        let particle_manager = Arc::new(Mutex::new(ParticleSystemManager::new(
            device.clone(),
            queue.clone(),
            (2000.0 * quality_level.particle_multiplier()) as usize,
        )));

        // Initialize animation system
        let animation_manager = Arc::new(Mutex::new(AnimationManager::new()));

        // Initialize enhanced audio system
        let audio_manager = EnhancedAudioManager::new().await?;

        // Initialize visual effects system
        let visual_effects =
            VisualEffectsManager::new(particle_manager.clone(), animation_manager.clone());

        // Initialize dynamic lighting
        let lighting_system = DynamicLighting::new(device.clone(), queue.clone());

        // Initialize performance management
        let performance_manager = AdaptivePerformanceManager::new(quality_level, 60.0);

        Ok(Self {
            particle_manager,
            animation_manager,
            audio_manager,
            visual_effects,
            lighting_system,
            performance_manager,
            current_time: 0.0,
            camera_position: Vec3::ZERO,
            camera_forward: Vec3::NEG_Z,
            camera_up: Vec3::Y,
            is_initialized: false,
        })
    }

    /// Initialize the effects system with game assets
    pub async fn initialize(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Loading C&C Generals effects assets...");

        // Load particle system templates from archives
        // In a full implementation, these would be loaded from INI files in the BIG archives
        self.load_particle_templates(archive_system).await?;

        // Load animation data
        self.load_animation_data(archive_system).await?;

        // Initialize audio with faction music
        self.initialize_audio_system(archive_system).await?;

        self.is_initialized = true;
        info!("Effects Integration System initialized successfully");

        Ok(())
    }

    /// Update all effects systems using WW3D timing
    pub fn update_with_timing(
        &mut self,
        timing: &FrameTiming,
        camera_pos: Vec3,
        camera_forward: Vec3,
        camera_up: Vec3,
    ) {
        if !self.is_initialized {
            return;
        }
        let delta_time = timing.delta_seconds().max(0.0);
        let absolute_time = timing.total_seconds();
        self.current_time = absolute_time;
        self.camera_position = camera_pos;
        self.camera_forward = camera_forward;
        self.camera_up = camera_up;

        self.performance_manager
            .update_with_timing(timing, camera_pos);

        self.update_internal(
            delta_time,
            absolute_time,
            camera_pos,
            camera_forward,
            camera_up,
        );
    }

    /// Update all effects systems with explicit timing (legacy path)
    pub fn update(
        &mut self,
        delta_time: f32,
        camera_pos: Vec3,
        camera_forward: Vec3,
        camera_up: Vec3,
    ) {
        if !self.is_initialized {
            return;
        }
        self.current_time += delta_time;
        let absolute_time = self.current_time;
        self.camera_position = camera_pos;
        self.camera_forward = camera_forward;
        self.camera_up = camera_up;

        self.performance_manager
            .update(delta_time, absolute_time, camera_pos);

        self.update_internal(
            delta_time,
            absolute_time,
            camera_pos,
            camera_forward,
            camera_up,
        );
    }

    fn update_internal(
        &mut self,
        delta_time: f32,
        absolute_time: f32,
        camera_pos: Vec3,
        camera_forward: Vec3,
        camera_up: Vec3,
    ) {
        let _lod_manager = self.performance_manager.get_lod_manager();

        let view_projection = Mat4::IDENTITY;
        if let Ok(mut manager) = self.particle_manager.lock() {
            manager.update(view_projection, camera_pos, delta_time);
        }

        if let Ok(mut manager) = self.animation_manager.lock() {
            manager.update(delta_time);
        }

        self.visual_effects.update(delta_time, camera_pos);
        self.lighting_system.update(delta_time, camera_pos);

        self.audio_manager.set_listener_transform(
            camera_pos,
            camera_forward,
            camera_up,
            Vec3::ZERO,
        );
        self.audio_manager
            .update_with_time(delta_time, absolute_time);

        let particles = if let Ok(manager) = self.particle_manager.lock() {
            manager.get_particle_count() as u32
        } else {
            0
        };
        let lights = self.lighting_system.get_active_light_count() as u32;
        let effects = self.visual_effects.active_effects.len() as u32;

        self.performance_manager
            .get_lod_manager_mut()
            .update_budgets(particles, lights, effects);
    }

    /// Render all visual effects
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
    ) {
        self.render_with_shadow_scene(encoder, view, depth_view, |_pass| {});
    }

    /// Render all visual effects with a caller-provided shadow-scene callback.
    pub fn render_with_shadow_scene(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        mut shadow_scene_render_fn: impl FnMut(&mut wgpu::RenderPass),
    ) {
        self.render_with_shadow_scene_context(
            encoder,
            view,
            depth_view,
            |_light, _layer, pass| shadow_scene_render_fn(pass),
        );
    }

    /// Render all visual effects with per-light shadow-scene callback context.
    pub fn render_with_shadow_scene_context(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        mut shadow_scene_render_fn: impl FnMut(
            &crate::effects::lighting_system::LightSource,
            usize,
            &mut wgpu::RenderPass,
        ),
    ) {
        if !self.is_initialized {
            return;
        }

        // Render shadow maps first
        self.lighting_system
            .render_shadow_maps_with_context(encoder, |light, layer, pass| {
                shadow_scene_render_fn(light, layer, pass)
            });

        // Render particles
        if let Ok(manager) = self.particle_manager.lock() {
            manager.render(encoder, view, depth_view);
        }

        // Apply dynamic lighting
        self.lighting_system.render_lighting(encoder, view);
    }

    /// Create an authentic C&C explosion effect
    pub async fn create_explosion(
        &mut self,
        position: Vec3,
        size: f32,
        explosion_type: &str,
        archive_system: &mut ArchiveFileSystem,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Check LOD settings
        let lod_manager = self.performance_manager.get_lod_manager();
        if !lod_manager.should_render_effect(
            position,
            crate::effects::particle_system::ParticlePriority::DeathExplosion,
        ) {
            return Ok(()); // Skip due to LOD
        }

        // Create appropriate explosion effect
        let explosion = match explosion_type {
            "tank" => VisualEffectsManager::create_tank_explosion(position),
            "building" => VisualEffectsManager::create_building_explosion(position, size),
            "nuclear" => VisualEffectsManager::create_nuclear_explosion(position),
            _ => VisualEffectsManager::create_tank_explosion(position), // Default
        };

        // Apply particle reduction for LOD
        let _particle_reduction = lod_manager.get_particle_reduction(position);

        // Trigger the explosion with all effects
        self.visual_effects
            .trigger_explosion(explosion, &mut self.audio_manager, archive_system)
            .await;

        // Add dynamic lighting
        let light_id = self.lighting_system.add_light(
            crate::effects::lighting_system::DynamicLighting::create_explosion_light(
                position, size,
            ),
        );

        debug!(
            "Created explosion '{}' at {:?} (size: {:.2}, light: {})",
            explosion_type, position, size, light_id
        );

        Ok(())
    }

    /// Create weapon firing effects (muzzle flash, sound, lighting)
    pub async fn create_weapon_fire(
        &mut self,
        weapon_type: &str,
        muzzle_position: Vec3,
        target_position: Option<Vec3>,
        _object_id: Option<ObjectId>,
        archive_system: &mut ArchiveFileSystem,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let lod_manager = self.performance_manager.get_lod_manager();
        if !lod_manager.should_render_effect(
            muzzle_position,
            crate::effects::particle_system::ParticlePriority::WeaponTrail,
        ) {
            return Ok(());
        }

        // Create weapon effect based on type
        let weapon_effect = match weapon_type {
            "tank_cannon" => {
                if let Some(target) = target_position {
                    VisualEffectsManager::create_tank_cannon_effect(muzzle_position, target)
                } else {
                    return Err("Tank cannon requires target position".into());
                }
            }
            "machine_gun" => VisualEffectsManager::create_machine_gun_effect(muzzle_position),
            "missile" => {
                if let Some(target) = target_position {
                    VisualEffectsManager::create_missile_effect(muzzle_position, target)
                } else {
                    return Err("Missile requires target position".into());
                }
            }
            _ => VisualEffectsManager::create_machine_gun_effect(muzzle_position), // Default
        };

        // Trigger weapon effect
        self.visual_effects
            .trigger_weapon_effect(weapon_effect, &mut self.audio_manager, archive_system)
            .await;

        // Add muzzle flash light
        let direction = if let Some(target) = target_position {
            (target - muzzle_position).normalize()
        } else {
            self.camera_forward
        };

        let light_id = self.lighting_system.add_light(
            crate::effects::lighting_system::DynamicLighting::create_muzzle_flash_light(
                muzzle_position,
                direction,
            ),
        );

        debug!(
            "Created weapon fire '{}' at {:?} (light: {})",
            weapon_type, muzzle_position, light_id
        );

        Ok(())
    }

    /// Play unit voice with authentic C&C characteristics
    pub async fn play_unit_voice(
        &mut self,
        unit_type: &str,
        voice_event: &str,
        faction: crate::effects::audio_integration::Faction,
        position: Option<Vec3>,
        archive_system: &mut ArchiveFileSystem,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let audio_event = match voice_event {
            "select" => crate::effects::audio_integration::AudioEventType::UnitSelect,
            "move" => crate::effects::audio_integration::AudioEventType::UnitMove,
            "attack" => crate::effects::audio_integration::AudioEventType::UnitAttack,
            "die" => crate::effects::audio_integration::AudioEventType::UnitDie,
            "create" => crate::effects::audio_integration::AudioEventType::UnitCreate,
            _ => return Err(format!("Unknown voice event: {}", voice_event).into()),
        };

        if let Some(pos) = position {
            // 3D positioned voice
            self.audio_manager
                .play_audio_event_3d(archive_system, audio_event, pos, None, Some(faction))
                .await?;
        } else {
            // 2D UI voice
            self.audio_manager
                .play_audio_event_2d(archive_system, audio_event, None)
                .await?;
        }

        debug!(
            "Played unit voice '{}' for {} {:?}",
            voice_event, unit_type, faction
        );
        Ok(())
    }

    /// Start unit animation (movement, attack, etc.)
    pub fn start_unit_animation(
        &mut self,
        object_id: ObjectId,
        animation_type: crate::effects::animation_system::AnimationType,
        replace_existing: bool,
    ) {
        if let Ok(mut manager) = self.animation_manager.lock() {
            manager.play_animation(
                object_id,
                &format!("{:?}", animation_type),
                replace_existing,
            );
        }
        debug!(
            "Started animation {:?} for object {:?}",
            animation_type, object_id
        );
    }

    /// Stop unit animation
    pub fn stop_unit_animation(
        &mut self,
        object_id: ObjectId,
        animation_type: crate::effects::animation_system::AnimationType,
    ) {
        if let Ok(mut manager) = self.animation_manager.lock() {
            manager.stop_animation(object_id, animation_type);
        }
        debug!(
            "Stopped animation {:?} for object {:?}",
            animation_type, object_id
        );
    }

    /// Create environmental effect (dust, smoke, etc.)
    pub async fn create_environmental_effect(
        &mut self,
        effect_type: &str,
        position: Vec3,
        _duration: f32,
        object_id: Option<ObjectId>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let lod_manager = self.performance_manager.get_lod_manager();
        if !lod_manager.should_render_effect(
            position,
            crate::effects::particle_system::ParticlePriority::Ambient,
        ) {
            return Ok(());
        }

        let visual_effect_type = match effect_type {
            "dust_trail" => crate::effects::visual_effects::EffectType::DustCloud,
            "smoke_trail" => crate::effects::visual_effects::EffectType::SmokeTrail,
            "construction_sparks" => crate::effects::visual_effects::EffectType::ConstructionSparks,
            "engine_exhaust" => crate::effects::visual_effects::EffectType::EngineExhaust,
            _ => return Err(format!("Unknown environmental effect: {}", effect_type).into()),
        };

        self.visual_effects
            .trigger_effect(visual_effect_type, position, 1.0, 0.0, object_id);

        debug!(
            "Created environmental effect '{}' at {:?}",
            effect_type, position
        );
        Ok(())
    }

    /// Set ambient lighting (time of day)
    pub fn set_ambient_lighting(&mut self, color: Vec3, intensity: f32) {
        self.lighting_system.set_ambient_lighting(color, intensity);
        info!(
            "Set ambient lighting: color={:?}, intensity={:.2}",
            color, intensity
        );
    }

    /// Play faction-appropriate music
    pub async fn play_faction_music(
        &mut self,
        faction: crate::effects::audio_integration::Faction,
        archive_system: &mut ArchiveFileSystem,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.audio_manager
            .play_faction_music(archive_system, faction)
            .await;
        info!("Started faction music for {:?}", faction);
        Ok(())
    }

    /// Set overall quality level
    pub fn set_quality_level(&mut self, quality: QualityLevel) {
        self.performance_manager
            .force_quality_level(quality, self.current_time);
        info!("Set effects quality level to {:?}", quality);
    }

    /// Get performance statistics
    pub fn get_performance_stats(&self) -> crate::effects::performance::PerformanceStats {
        self.performance_manager
            .get_lod_manager()
            .get_performance_stats()
    }

    /// Get current FPS
    pub fn get_current_fps(&self) -> f32 {
        self.performance_manager.get_current_fps()
    }

    /// Get screen shake offset for camera
    pub fn get_screen_shake(&self) -> Vec3 {
        self.visual_effects.get_screen_shake()
    }

    /// Get camera flash effect
    pub fn get_camera_flash(&self) -> (Vec3, f32) {
        self.visual_effects.get_camera_flash()
    }

    /// Enable/disable auto quality adjustment
    pub fn set_auto_quality_adjustment(&mut self, enabled: bool) {
        self.performance_manager.set_auto_adjust(enabled);
    }

    /// Set master audio volume
    pub fn set_master_volume(&mut self, volume: f32) {
        self.audio_manager.set_master_volume(volume);
    }

    /// Set music volume
    pub fn set_music_volume(&mut self, volume: f32) {
        self.audio_manager.set_music_volume(volume);
    }

    /// Set sound effects volume
    pub fn set_sfx_volume(&mut self, volume: f32) {
        self.audio_manager.set_sfx_volume(volume);
    }

    /// Set voice volume
    pub fn set_voice_volume(&mut self, volume: f32) {
        self.audio_manager.set_voice_volume(volume);
    }

    /// Private helper functions

    async fn load_particle_templates(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Load particle system templates from archive
        let templates = crate::effects::particle_system::load_particle_templates(
            archive_system,
            "ParticleSystems.ini",
        )
        .await?;

        for template in templates {
            if let Ok(mut manager) = self.particle_manager.lock() {
                manager.add_template(template);
            }
        }

        info!("Loaded particle system templates");
        Ok(())
    }

    async fn load_animation_data(
        &mut self,
        _archive_system: &mut ArchiveFileSystem,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // In a full implementation, this would load animation data from archives
        info!("Animation data loaded (using defaults)");
        Ok(())
    }

    async fn initialize_audio_system(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Test audio system by playing a random music track
        self.audio_manager
            .play_random_cnc_music(archive_system)
            .await;

        info!("Audio system initialized");
        Ok(())
    }
}

/// Convenience functions for common effect combinations
impl EffectsIntegration {
    /// Create a complete tank destruction sequence
    pub async fn tank_destroyed(
        &mut self,
        position: Vec3,
        _tank_type: &str,
        archive_system: &mut ArchiveFileSystem,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Main explosion
        self.create_explosion(position, 1.0, "tank", archive_system)
            .await?;

        // Secondary debris effects (if LOD allows)
        let lod_manager = self.performance_manager.get_lod_manager();
        if !lod_manager.should_skip_secondary_effects(position) {
            // Add debris trail
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            self.create_environmental_effect("dust_trail", position, 3.0, None)
                .await?;
        }

        Ok(())
    }

    /// Create a building construction sequence
    pub async fn building_constructed(
        &mut self,
        position: Vec3,
        building_type: &str,
        object_id: ObjectId,
        archive_system: &mut ArchiveFileSystem,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Start construction animation
        self.start_unit_animation(
            object_id,
            crate::effects::animation_system::AnimationType::BuildingConstruct,
            true,
        );

        // Construction sparks effect
        self.create_environmental_effect("construction_sparks", position, 5.0, Some(object_id))
            .await?;

        // Construction audio
        let _ = self
            .audio_manager
            .play_audio_event_3d(
                archive_system,
                crate::effects::audio_integration::AudioEventType::BuildingConstruct,
                position,
                Some(object_id),
                None,
            )
            .await;

        info!(
            "Started construction sequence for {} at {:?}",
            building_type, position
        );
        Ok(())
    }

    /// Create a unit movement with dust trail
    pub async fn unit_moving(
        &mut self,
        object_id: ObjectId,
        position: Vec3,
        unit_type: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Start movement animation
        self.start_unit_animation(
            object_id,
            crate::effects::animation_system::AnimationType::UnitMove,
            true,
        );

        // Add dust trail for ground units
        if unit_type.contains("tank") || unit_type.contains("vehicle") {
            self.create_environmental_effect("dust_trail", position, 1.0, Some(object_id))
                .await?;
        }

        Ok(())
    }

    /// Stop all effects for an object (when destroyed/removed)
    pub fn stop_all_object_effects(&mut self, object_id: ObjectId) {
        // Stop animations
        if let Ok(mut manager) = self.animation_manager.lock() {
            manager.stop_animation(
                object_id,
                crate::effects::animation_system::AnimationType::UnitMove,
            );
            manager.stop_animation(
                object_id,
                crate::effects::animation_system::AnimationType::BuildingConstruct,
            );
        }

        // Stop audio
        self.audio_manager.stop_object_audio(object_id);

        debug!("Stopped all effects for object {:?}", object_id);
    }
}
