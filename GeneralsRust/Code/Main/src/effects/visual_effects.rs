use glam::{Vec3, Vec4};
use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::Arc;

use crate::assets::archive::ArchiveFileSystem;
use crate::effects::animation_system::AnimationManager;
use crate::effects::audio_integration::{AudioEventType, EnhancedAudioManager};
use crate::effects::particle_system::ParticleSystemManager;
use crate::game_logic::ObjectId;

/// Visual effect types matching C&C categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectType {
    // Explosion effects
    TankExplosion,
    BuildingExplosion,
    MissileExplosion,
    BarrelExplosion,
    NuclearExplosion,

    // Weapon effects
    MuzzleFlash,
    LaserBeam,
    MissileTrail,
    BulletTracer,
    ShellCasing,

    // Impact effects
    BulletImpactDirt,
    BulletImpactMetal,
    BulletImpactConcrete,
    ExplosionScorch,
    CraterFormation,

    // Environmental effects
    SmokeTrail,
    DustCloud,
    FireBurst,
    WaterSplash,
    DebrisScatter,

    // Unit effects
    UnitDamageSmoke,
    EngineExhaust,
    JetTrail,
    HelicopterWash,
    TankTreadDust,

    // Building effects
    ConstructionSparks,
    PowerPlantGlow,
    RadarSweep,
    FactorySmoke,

    // Special effects
    TeleportEffect,
    ShieldHit,
    ElectricArc,
    HealingEffect,
    RepairSparks,
}

/// Effect trigger conditions
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EffectTrigger {
    Immediate,
    OnImpact,
    OnDestroy,
    Continuous,
    OnDamage,
    OnHeal,
    OnAttack,
}

/// Visual effect definition
#[derive(Debug, Clone)]
pub struct VisualEffect {
    pub effect_type: EffectType,
    pub particle_template_name: String,
    pub animation_name: Option<String>,
    pub audio_event: Option<AudioEventType>,
    pub duration: f32,
    pub trigger: EffectTrigger,
    pub scale_factor: f32,
    pub color_tint: Vec4, // RGBA multiplier
    pub attach_to_source: bool,
    pub billboard_to_camera: bool,
    pub fade_in_time: f32,
    pub fade_out_time: f32,
    pub delay: f32,
    pub light_emission: Option<LightEmission>,
    pub screen_shake: Option<ScreenShakeParams>,
    pub camera_effects: Option<CameraEffectParams>,
}

/// Light emission properties for effects
#[derive(Debug, Clone)]
pub struct LightEmission {
    pub color: Vec3, // RGB
    pub intensity: f32,
    pub radius: f32,
    pub falloff: f32,
    pub flicker_speed: f32,
    pub flicker_intensity: f32,
}

/// Screen shake parameters
#[derive(Debug, Clone)]
pub struct ScreenShakeParams {
    pub intensity: f32,
    pub duration: f32,
    pub frequency: f32,
    pub falloff_distance: f32,
}

/// Camera effect parameters
#[derive(Debug, Clone)]
pub struct CameraEffectParams {
    pub flash_color: Vec3,
    pub flash_intensity: f32,
    pub flash_duration: f32,
    pub distortion_strength: f32,
    pub chromatic_aberration: f32,
}

/// Active visual effect instance
#[derive(Debug)]
pub struct ActiveEffect {
    pub effect_type: EffectType,
    pub particle_system_id: Option<usize>,
    pub position: Vec3,
    pub rotation: f32,
    pub scale: f32,
    pub start_time: f32,
    pub duration: f32,
    pub object_id: Option<ObjectId>,
    pub light_emission: Option<LightEmission>,
    pub screen_shake: Option<ScreenShakeParams>,
    pub camera_effects: Option<CameraEffectParams>,
    pub is_active: bool,
    pub fade_progress: f32,
}

#[derive(Debug, Clone)]
struct ScheduledImpulse {
    execute_at: f32,
    kind: ScheduledImpulseKind,
}

#[derive(Debug, Clone)]
enum ScheduledImpulseKind {
    ScreenShake {
        params: ScreenShakeParams,
        position: Vec3,
    },
    Light {
        position: Vec3,
        light: LightEmission,
        duration: f32,
    },
}

#[derive(Debug, Clone)]
struct ActiveLight {
    position: Vec3,
    light: LightEmission,
    start_time: f32,
    duration: f32,
}

/// Explosion effect with multiple stages
pub struct ExplosionEffect {
    pub size: f32, // 0.1 = small, 1.0 = medium, 2.0+ = large
    pub position: Vec3,
    pub explosion_type: EffectType,
    pub stages: Vec<ExplosionStage>,
}

#[derive(Debug, Clone)]
pub struct ExplosionStage {
    pub delay: f32,
    pub particle_template: String,
    pub animation: Option<String>,
    pub audio_event: Option<AudioEventType>,
    pub light_params: Option<LightEmission>,
    pub shake_params: Option<ScreenShakeParams>,
}

impl ExplosionEffect {
    pub fn tank_explosion(position: Vec3) -> Self {
        Self {
            size: 1.0,
            position,
            explosion_type: EffectType::TankExplosion,
            stages: vec![
                // Initial flash
                ExplosionStage {
                    delay: 0.0,
                    particle_template: "ExplosionFlash".to_string(),
                    animation: Some("MuzzleFlash".to_string()),
                    audio_event: Some(AudioEventType::ExplosionMedium),
                    light_params: Some(LightEmission {
                        color: Vec3::new(1.0, 0.8, 0.4),
                        intensity: 5.0,
                        radius: 20.0,
                        falloff: 2.0,
                        flicker_speed: 0.0,
                        flicker_intensity: 0.0,
                    }),
                    shake_params: Some(ScreenShakeParams {
                        intensity: 0.3,
                        duration: 0.8,
                        frequency: 15.0,
                        falloff_distance: 50.0,
                    }),
                },
                // Fire and smoke
                ExplosionStage {
                    delay: 0.1,
                    particle_template: "ExplosionFire".to_string(),
                    animation: None,
                    audio_event: None,
                    light_params: Some(LightEmission {
                        color: Vec3::new(1.0, 0.3, 0.0),
                        intensity: 3.0,
                        radius: 15.0,
                        falloff: 1.5,
                        flicker_speed: 8.0,
                        flicker_intensity: 0.3,
                    }),
                    shake_params: None,
                },
                // Debris and sparks
                ExplosionStage {
                    delay: 0.2,
                    particle_template: "ExplosionDebris".to_string(),
                    animation: None,
                    audio_event: None,
                    light_params: None,
                    shake_params: None,
                },
                // Lingering smoke
                ExplosionStage {
                    delay: 1.0,
                    particle_template: "ExplosionSmoke".to_string(),
                    animation: None,
                    audio_event: None,
                    light_params: None,
                    shake_params: None,
                },
            ],
        }
    }

    pub fn building_explosion(position: Vec3, size: f32) -> Self {
        Self {
            size,
            position,
            explosion_type: EffectType::BuildingExplosion,
            stages: vec![
                // Main explosion
                ExplosionStage {
                    delay: 0.0,
                    particle_template: "BuildingExplosionCore".to_string(),
                    animation: None,
                    audio_event: Some(AudioEventType::ExplosionLarge),
                    light_params: Some(LightEmission {
                        color: Vec3::new(1.0, 0.7, 0.3),
                        intensity: 8.0 * size,
                        radius: 30.0 * size,
                        falloff: 1.8,
                        flicker_speed: 0.0,
                        flicker_intensity: 0.0,
                    }),
                    shake_params: Some(ScreenShakeParams {
                        intensity: 0.5 * size,
                        duration: 1.2,
                        frequency: 12.0,
                        falloff_distance: 80.0,
                    }),
                },
                // Secondary explosions
                ExplosionStage {
                    delay: 0.3,
                    particle_template: "BuildingDebris".to_string(),
                    animation: None,
                    audio_event: Some(AudioEventType::ExplosionMedium),
                    light_params: None,
                    shake_params: Some(ScreenShakeParams {
                        intensity: 0.2,
                        duration: 0.6,
                        frequency: 20.0,
                        falloff_distance: 60.0,
                    }),
                },
                // Dust and smoke
                ExplosionStage {
                    delay: 0.8,
                    particle_template: "BuildingDust".to_string(),
                    animation: None,
                    audio_event: None,
                    light_params: None,
                    shake_params: None,
                },
            ],
        }
    }

    pub fn nuclear_explosion(position: Vec3) -> Self {
        Self {
            size: 5.0,
            position,
            explosion_type: EffectType::NuclearExplosion,
            stages: vec![
                // Initial flash (blinding white)
                ExplosionStage {
                    delay: 0.0,
                    particle_template: "NuclearFlash".to_string(),
                    animation: None,
                    audio_event: Some(AudioEventType::ExplosionLarge),
                    light_params: Some(LightEmission {
                        color: Vec3::new(2.0, 2.0, 2.0), // Overblown white
                        intensity: 20.0,
                        radius: 200.0,
                        falloff: 0.5,
                        flicker_speed: 0.0,
                        flicker_intensity: 0.0,
                    }),
                    shake_params: Some(ScreenShakeParams {
                        intensity: 1.0,
                        duration: 3.0,
                        frequency: 8.0,
                        falloff_distance: 300.0,
                    }),
                },
                // Fireball expansion
                ExplosionStage {
                    delay: 0.5,
                    particle_template: "NuclearFireball".to_string(),
                    animation: None,
                    audio_event: None,
                    light_params: Some(LightEmission {
                        color: Vec3::new(1.0, 0.5, 0.1),
                        intensity: 15.0,
                        radius: 150.0,
                        falloff: 1.0,
                        flicker_speed: 5.0,
                        flicker_intensity: 0.2,
                    }),
                    shake_params: None,
                },
                // Mushroom cloud
                ExplosionStage {
                    delay: 2.0,
                    particle_template: "NuclearMushroom".to_string(),
                    animation: None,
                    audio_event: None,
                    light_params: Some(LightEmission {
                        color: Vec3::new(0.8, 0.4, 0.2),
                        intensity: 8.0,
                        radius: 100.0,
                        falloff: 1.2,
                        flicker_speed: 2.0,
                        flicker_intensity: 0.4,
                    }),
                    shake_params: None,
                },
                // Radiation glow (lingering)
                ExplosionStage {
                    delay: 5.0,
                    particle_template: "RadiationGlow".to_string(),
                    animation: None,
                    audio_event: None,
                    light_params: Some(LightEmission {
                        color: Vec3::new(0.2, 1.0, 0.2),
                        intensity: 2.0,
                        radius: 80.0,
                        falloff: 2.0,
                        flicker_speed: 3.0,
                        flicker_intensity: 0.6,
                    }),
                    shake_params: None,
                },
            ],
        }
    }
}

/// Weapon effect for muzzle flashes and projectiles
pub struct WeaponEffect {
    pub weapon_type: String,
    pub muzzle_position: Vec3,
    pub target_position: Option<Vec3>,
    pub effects: Vec<WeaponEffectStage>,
}

#[derive(Debug, Clone)]
pub struct WeaponEffectStage {
    pub delay: f32,
    pub effect_type: EffectType,
    pub position_offset: Vec3,
    pub particle_template: String,
    pub audio_event: Option<AudioEventType>,
    pub light_params: Option<LightEmission>,
    pub duration: f32,
}

impl WeaponEffect {
    pub fn tank_cannon(muzzle_pos: Vec3, target_pos: Vec3) -> Self {
        Self {
            weapon_type: "TankCannon".to_string(),
            muzzle_position: muzzle_pos,
            target_position: Some(target_pos),
            effects: vec![
                // Muzzle flash
                WeaponEffectStage {
                    delay: 0.0,
                    effect_type: EffectType::MuzzleFlash,
                    position_offset: Vec3::ZERO,
                    particle_template: "TankMuzzleFlash".to_string(),
                    audio_event: Some(AudioEventType::WeaponFire),
                    light_params: Some(LightEmission {
                        color: Vec3::new(1.0, 0.8, 0.4),
                        intensity: 3.0,
                        radius: 8.0,
                        falloff: 3.0,
                        flicker_speed: 0.0,
                        flicker_intensity: 0.0,
                    }),
                    duration: 0.3,
                },
                // Shell casing
                WeaponEffectStage {
                    delay: 0.1,
                    effect_type: EffectType::ShellCasing,
                    position_offset: Vec3::new(-0.5, 0.0, -0.2),
                    particle_template: "ShellCasing".to_string(),
                    audio_event: None,
                    light_params: None,
                    duration: 2.0,
                },
                // Smoke puff
                WeaponEffectStage {
                    delay: 0.2,
                    effect_type: EffectType::SmokeTrail,
                    position_offset: Vec3::ZERO,
                    particle_template: "MuzzleSmoke".to_string(),
                    audio_event: None,
                    light_params: None,
                    duration: 1.5,
                },
            ],
        }
    }

    pub fn machine_gun(muzzle_pos: Vec3) -> Self {
        Self {
            weapon_type: "MachineGun".to_string(),
            muzzle_position: muzzle_pos,
            target_position: None,
            effects: vec![
                // Small muzzle flash
                WeaponEffectStage {
                    delay: 0.0,
                    effect_type: EffectType::MuzzleFlash,
                    position_offset: Vec3::ZERO,
                    particle_template: "SmallMuzzleFlash".to_string(),
                    audio_event: Some(AudioEventType::WeaponFire),
                    light_params: Some(LightEmission {
                        color: Vec3::new(1.0, 0.9, 0.6),
                        intensity: 1.5,
                        radius: 3.0,
                        falloff: 4.0,
                        flicker_speed: 0.0,
                        flicker_intensity: 0.0,
                    }),
                    duration: 0.1,
                },
                // Bullet tracer
                WeaponEffectStage {
                    delay: 0.0,
                    effect_type: EffectType::BulletTracer,
                    position_offset: Vec3::ZERO,
                    particle_template: "BulletTracer".to_string(),
                    audio_event: None,
                    light_params: None,
                    duration: 0.2,
                },
            ],
        }
    }

    pub fn missile_launcher(muzzle_pos: Vec3, target_pos: Vec3) -> Self {
        Self {
            weapon_type: "MissileLauncher".to_string(),
            muzzle_position: muzzle_pos,
            target_position: Some(target_pos),
            effects: vec![
                // Launch flash
                WeaponEffectStage {
                    delay: 0.0,
                    effect_type: EffectType::MuzzleFlash,
                    position_offset: Vec3::ZERO,
                    particle_template: "MissileLaunchFlash".to_string(),
                    audio_event: Some(AudioEventType::WeaponFire),
                    light_params: Some(LightEmission {
                        color: Vec3::new(0.8, 0.8, 1.0),
                        intensity: 4.0,
                        radius: 10.0,
                        falloff: 2.0,
                        flicker_speed: 0.0,
                        flicker_intensity: 0.0,
                    }),
                    duration: 0.5,
                },
                // Missile trail
                WeaponEffectStage {
                    delay: 0.1,
                    effect_type: EffectType::MissileTrail,
                    position_offset: Vec3::ZERO,
                    particle_template: "MissileTrail".to_string(),
                    audio_event: Some(AudioEventType::MissileTrail),
                    light_params: Some(LightEmission {
                        color: Vec3::new(0.6, 0.8, 1.0),
                        intensity: 2.0,
                        radius: 5.0,
                        falloff: 1.5,
                        flicker_speed: 10.0,
                        flicker_intensity: 0.3,
                    }),
                    duration: 3.0, // Missile flight time
                },
            ],
        }
    }
}

/// Visual effects manager coordinates all visual effects
pub struct VisualEffectsManager {
    effects: HashMap<EffectType, VisualEffect>,
    pub(crate) active_effects: Vec<ActiveEffect>,
    particle_manager: Arc<std::sync::Mutex<ParticleSystemManager>>,
    animation_manager: Arc<std::sync::Mutex<AnimationManager>>,
    scheduled_impulses: Vec<ScheduledImpulse>,
    active_lights: Vec<ActiveLight>,
    current_time: f32,
    camera_position: Vec3,
    screen_shake_accumulator: Vec3,
    camera_flash_intensity: f32,
    camera_flash_color: Vec3,
    max_concurrent_effects: usize,
}

impl VisualEffectsManager {
    pub fn new(
        particle_manager: Arc<std::sync::Mutex<ParticleSystemManager>>,
        animation_manager: Arc<std::sync::Mutex<AnimationManager>>,
    ) -> Self {
        let mut manager = Self {
            effects: HashMap::new(),
            active_effects: Vec::new(),
            particle_manager,
            animation_manager,
            scheduled_impulses: Vec::new(),
            active_lights: Vec::new(),
            current_time: 0.0,
            camera_position: Vec3::ZERO,
            screen_shake_accumulator: Vec3::ZERO,
            camera_flash_intensity: 0.0,
            camera_flash_color: Vec3::ZERO,
            max_concurrent_effects: 200,
        };

        manager.load_default_effects();
        manager
    }

    /// Trigger a visual effect at a position
    pub fn trigger_effect(
        &mut self,
        effect_type: EffectType,
        position: Vec3,
        scale: f32,
        rotation: f32,
        object_id: Option<ObjectId>,
    ) {
        if let Some(effect_def) = self.effects.get(&effect_type).cloned() {
            // Create particle system if specified
            let particle_system_id = if !effect_def.particle_template_name.is_empty() {
                let mut manager = self
                    .particle_manager
                    .lock()
                    .expect("Particle system manager mutex poisoned");
                let id = manager.create_system(&effect_def.particle_template_name);
                if let Some(system_id) = id {
                    if let Some(system) = manager.get_system_mut(system_id) {
                        system.set_position(position);
                        system.size_multiplier = scale * effect_def.scale_factor;
                        system.trigger();
                        system.start();
                    }
                }
                id
            } else {
                None
            };

            // Create active effect instance
            let active_effect = ActiveEffect {
                effect_type,
                particle_system_id,
                position,
                rotation,
                scale: scale * effect_def.scale_factor,
                start_time: self.current_time,
                duration: effect_def.duration,
                object_id,
                light_emission: effect_def.light_emission.clone(),
                screen_shake: effect_def.screen_shake.clone(),
                camera_effects: effect_def.camera_effects.clone(),
                is_active: true,
                fade_progress: 0.0,
            };

            self.active_effects.push(active_effect);

            // Apply immediate effects
            if let Some(screen_shake) = &effect_def.screen_shake {
                self.apply_screen_shake(screen_shake, position);
            }

            if let Some(camera_effects) = &effect_def.camera_effects {
                self.apply_camera_flash(camera_effects);
            }

            debug!(
                "Triggered visual effect {:?} at {:?}",
                effect_type, position
            );
        } else {
            warn!("Visual effect {:?} not defined", effect_type);
        }
    }

    /// Trigger an explosion effect
    pub async fn trigger_explosion(
        &mut self,
        explosion: ExplosionEffect,
        audio_manager: &mut EnhancedAudioManager,
        archive_system: &mut ArchiveFileSystem,
    ) {
        info!(
            "Triggering explosion {:?} at {:?}",
            explosion.explosion_type, explosion.position
        );

        for stage in &explosion.stages {
            let delay_seconds = stage.delay.max(0.0);
            let execute_at = self.current_time + delay_seconds;

            if !stage.particle_template.is_empty() {
                let mut manager = self
                    .particle_manager
                    .lock()
                    .expect("Particle system manager mutex poisoned");
                if let Some(system_id) = manager.create_system(&stage.particle_template) {
                    if let Some(system) = manager.get_system_mut(system_id) {
                        system.set_position(explosion.position);
                        system.size_multiplier = explosion.size.max(0.1);
                        let delay_frames = (delay_seconds * 60.0).round() as u32;
                        system.initial_delay_left =
                            system.initial_delay_left.saturating_add(delay_frames);
                        system.trigger();
                        system.start();
                    }
                }
            }

            if let Some(audio_event) = stage.audio_event {
                let _ = audio_manager
                    .play_audio_event_3d_with_delay(
                        archive_system,
                        audio_event,
                        explosion.position,
                        None,
                        None,
                        delay_seconds,
                    )
                    .await;
            }

            if let Some(light_params) = &stage.light_params {
                self.scheduled_impulses.push(ScheduledImpulse {
                    execute_at,
                    kind: ScheduledImpulseKind::Light {
                        position: explosion.position,
                        light: light_params.clone(),
                        duration: 1.0,
                    },
                });
            }

            if let Some(shake_params) = &stage.shake_params {
                self.scheduled_impulses.push(ScheduledImpulse {
                    execute_at,
                    kind: ScheduledImpulseKind::ScreenShake {
                        params: shake_params.clone(),
                        position: explosion.position,
                    },
                });
            }
        }
    }

    /// Trigger a weapon effect
    pub async fn trigger_weapon_effect(
        &mut self,
        weapon: WeaponEffect,
        audio_manager: &mut EnhancedAudioManager,
        archive_system: &mut ArchiveFileSystem,
    ) {
        debug!(
            "Triggering weapon effect {} at {:?}",
            weapon.weapon_type, weapon.muzzle_position
        );

        for stage in &weapon.effects {
            let effect_position = weapon.muzzle_position + stage.position_offset;

            // Trigger visual effect
            self.trigger_effect(stage.effect_type, effect_position, 1.0, 0.0, None);

            // Play audio
            if let Some(audio_event) = stage.audio_event {
                let _ = audio_manager
                    .play_audio_event_3d(archive_system, audio_event, effect_position, None, None)
                    .await;
            }

            // Apply lighting
            if let Some(light_params) = &stage.light_params {
                self.add_light_source(
                    effect_position,
                    light_params.clone(),
                    stage.duration.max(0.0),
                );
            }
        }
    }

    /// Update all active effects
    pub fn update(&mut self, delta_time: f32, camera_pos: Vec3) {
        self.current_time += delta_time;
        self.camera_position = camera_pos;

        // Execute scheduled impulses (delayed stage effects).
        if !self.scheduled_impulses.is_empty() {
            let now = self.current_time;
            let impulses = std::mem::take(&mut self.scheduled_impulses);
            for impulse in impulses {
                if impulse.execute_at > now {
                    self.scheduled_impulses.push(impulse);
                    continue;
                }
                match impulse.kind {
                    ScheduledImpulseKind::ScreenShake { params, position } => {
                        self.apply_screen_shake(&params, position);
                    }
                    ScheduledImpulseKind::Light {
                        position,
                        light,
                        duration,
                    } => {
                        self.add_light_source(position, light, duration);
                    }
                }
            }
        }

        // Update active effects
        for effect in &mut self.active_effects {
            if !effect.is_active {
                continue;
            }

            let elapsed = self.current_time - effect.start_time;

            // Check if effect is finished
            if elapsed >= effect.duration {
                effect.is_active = false;
                if let Some(system_id) = effect.particle_system_id {
                    if let Ok(mut manager) = self.particle_manager.lock() {
                        manager.destroy_system(system_id);
                    }
                }
                continue;
            }

            // Update fade progress
            effect.fade_progress = elapsed / effect.duration;

            // Update light emission if present
            if let Some(light) = &mut effect.light_emission {
                // Apply flickering
                if light.flicker_speed > 0.0 {
                    let flicker = (self.current_time * light.flicker_speed).sin();
                    light.intensity *= 1.0 + flicker * light.flicker_intensity;
                }
            }
        }

        // Remove finished effects
        self.active_effects.retain(|effect| effect.is_active);

        // Remove finished lights
        self.active_lights
            .retain(|light| self.current_time - light.start_time < light.duration);

        // Limit concurrent effects for performance
        if self.active_effects.len() > self.max_concurrent_effects {
            self.active_effects.sort_by(|a, b| {
                b.start_time
                    .partial_cmp(&a.start_time)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            self.active_effects.truncate(self.max_concurrent_effects);
        }

        // Update screen shake decay
        self.screen_shake_accumulator *= 0.9; // Exponential decay

        // Update camera flash decay
        self.camera_flash_intensity *= 0.95;
        if self.camera_flash_intensity < 0.01 {
            self.camera_flash_intensity = 0.0;
        }
    }

    fn apply_screen_shake(&mut self, shake_params: &ScreenShakeParams, effect_position: Vec3) {
        let distance = self.camera_position.distance(effect_position);
        let distance_falloff = if distance <= shake_params.falloff_distance {
            1.0 - (distance / shake_params.falloff_distance)
        } else {
            0.0
        };

        let shake_intensity = shake_params.intensity * distance_falloff;

        // Add random shake displacement
        let shake_x = (fastrand::f32() - 0.5) * shake_intensity;
        let shake_y = (fastrand::f32() - 0.5) * shake_intensity;
        let shake_z = (fastrand::f32() - 0.5) * shake_intensity * 0.5; // Less Z shake

        self.screen_shake_accumulator += Vec3::new(shake_x, shake_y, shake_z);
    }

    fn apply_camera_flash(&mut self, camera_effects: &CameraEffectParams) {
        self.camera_flash_intensity = camera_effects.flash_intensity;
        self.camera_flash_color = camera_effects.flash_color;
    }

    fn add_light_source(&mut self, position: Vec3, light: LightEmission, duration: f32) {
        self.active_lights.push(ActiveLight {
            position,
            light,
            start_time: self.current_time,
            duration: duration.max(0.0),
        });
        debug!("Added light source at {:?}", position);
    }

    /// Get current screen shake offset
    pub fn get_screen_shake(&self) -> Vec3 {
        self.screen_shake_accumulator
    }

    /// Get current camera flash effect
    pub fn get_camera_flash(&self) -> (Vec3, f32) {
        (self.camera_flash_color, self.camera_flash_intensity)
    }

    /// Get all active light sources for rendering
    pub fn get_active_lights(&self) -> Vec<(Vec3, LightEmission)> {
        let mut lights: Vec<(Vec3, LightEmission)> = self
            .active_effects
            .iter()
            .filter_map(|effect| {
                effect
                    .light_emission
                    .as_ref()
                    .map(|light| (effect.position, light.clone()))
            })
            .collect();

        for light in &self.active_lights {
            lights.push((light.position, light.light.clone()));
        }

        lights
    }

    fn load_default_effects(&mut self) {
        info!("Loading default visual effects");

        // Tank explosion
        self.effects.insert(
            EffectType::TankExplosion,
            VisualEffect {
                effect_type: EffectType::TankExplosion,
                particle_template_name: "TankExplosion".to_string(),
                animation_name: None,
                audio_event: Some(AudioEventType::ExplosionMedium),
                duration: 3.0,
                trigger: EffectTrigger::Immediate,
                scale_factor: 1.0,
                color_tint: Vec4::ONE,
                attach_to_source: false,
                billboard_to_camera: true,
                fade_in_time: 0.1,
                fade_out_time: 1.0,
                delay: 0.0,
                light_emission: Some(LightEmission {
                    color: Vec3::new(1.0, 0.6, 0.2),
                    intensity: 4.0,
                    radius: 25.0,
                    falloff: 2.0,
                    flicker_speed: 8.0,
                    flicker_intensity: 0.3,
                }),
                screen_shake: Some(ScreenShakeParams {
                    intensity: 0.4,
                    duration: 1.0,
                    frequency: 15.0,
                    falloff_distance: 60.0,
                }),
                camera_effects: Some(CameraEffectParams {
                    flash_color: Vec3::new(1.0, 0.8, 0.4),
                    flash_intensity: 0.3,
                    flash_duration: 0.2,
                    distortion_strength: 0.1,
                    chromatic_aberration: 0.02,
                }),
            },
        );

        // Muzzle flash
        self.effects.insert(
            EffectType::MuzzleFlash,
            VisualEffect {
                effect_type: EffectType::MuzzleFlash,
                particle_template_name: "MuzzleFlash".to_string(),
                animation_name: Some("MuzzleFlash".to_string()),
                audio_event: None, // Handled by weapon system
                duration: 0.2,
                trigger: EffectTrigger::Immediate,
                scale_factor: 1.0,
                color_tint: Vec4::ONE,
                attach_to_source: true,
                billboard_to_camera: true,
                fade_in_time: 0.0,
                fade_out_time: 0.15,
                delay: 0.0,
                light_emission: Some(LightEmission {
                    color: Vec3::new(1.0, 0.9, 0.6),
                    intensity: 3.0,
                    radius: 8.0,
                    falloff: 3.0,
                    flicker_speed: 0.0,
                    flicker_intensity: 0.0,
                }),
                screen_shake: None,
                camera_effects: None,
            },
        );

        // Dust trail
        self.effects.insert(
            EffectType::DustCloud,
            VisualEffect {
                effect_type: EffectType::DustCloud,
                particle_template_name: "DustTrail".to_string(),
                animation_name: None,
                audio_event: None,
                duration: 2.0,
                trigger: EffectTrigger::Continuous,
                scale_factor: 1.0,
                color_tint: Vec4::new(0.8, 0.7, 0.6, 0.7), // Dusty brown tint
                attach_to_source: true,
                billboard_to_camera: false,
                fade_in_time: 0.3,
                fade_out_time: 0.8,
                delay: 0.0,
                light_emission: None,
                screen_shake: None,
                camera_effects: None,
            },
        );

        // Building explosion
        self.effects.insert(
            EffectType::BuildingExplosion,
            VisualEffect {
                effect_type: EffectType::BuildingExplosion,
                particle_template_name: "TankExplosion".to_string(), // Reuse for now
                animation_name: None,
                audio_event: Some(AudioEventType::ExplosionLarge),
                duration: 5.0,
                trigger: EffectTrigger::Immediate,
                scale_factor: 2.0,
                color_tint: Vec4::ONE,
                attach_to_source: false,
                billboard_to_camera: true,
                fade_in_time: 0.2,
                fade_out_time: 2.0,
                delay: 0.0,
                light_emission: Some(LightEmission {
                    color: Vec3::new(1.0, 0.5, 0.1),
                    intensity: 8.0,
                    radius: 50.0,
                    falloff: 1.5,
                    flicker_speed: 5.0,
                    flicker_intensity: 0.4,
                }),
                screen_shake: Some(ScreenShakeParams {
                    intensity: 0.8,
                    duration: 2.0,
                    frequency: 12.0,
                    falloff_distance: 100.0,
                }),
                camera_effects: Some(CameraEffectParams {
                    flash_color: Vec3::new(1.0, 0.6, 0.2),
                    flash_intensity: 0.5,
                    flash_duration: 0.3,
                    distortion_strength: 0.2,
                    chromatic_aberration: 0.03,
                }),
            },
        );

        // Construction sparks
        self.effects.insert(
            EffectType::ConstructionSparks,
            VisualEffect {
                effect_type: EffectType::ConstructionSparks,
                particle_template_name: "DustTrail".to_string(), // Reuse for sparks-like effect
                animation_name: None,
                audio_event: None,
                duration: 0.5,
                trigger: EffectTrigger::Continuous,
                scale_factor: 0.5,
                color_tint: Vec4::new(1.0, 0.8, 0.2, 1.0), // Sparky yellow-orange
                attach_to_source: true,
                billboard_to_camera: false,
                fade_in_time: 0.0,
                fade_out_time: 0.3,
                delay: 0.0,
                light_emission: Some(LightEmission {
                    color: Vec3::new(1.0, 0.8, 0.4),
                    intensity: 1.5,
                    radius: 5.0,
                    falloff: 2.0,
                    flicker_speed: 15.0,
                    flicker_intensity: 0.6,
                }),
                screen_shake: None,
                camera_effects: None,
            },
        );

        info!("Loaded {} visual effects", self.effects.len());
    }

    /// Create common explosion effects
    pub fn create_tank_explosion(position: Vec3) -> ExplosionEffect {
        ExplosionEffect::tank_explosion(position)
    }

    pub fn create_building_explosion(position: Vec3, size: f32) -> ExplosionEffect {
        ExplosionEffect::building_explosion(position, size)
    }

    pub fn create_nuclear_explosion(position: Vec3) -> ExplosionEffect {
        ExplosionEffect::nuclear_explosion(position)
    }

    /// Create common weapon effects
    pub fn create_tank_cannon_effect(muzzle_pos: Vec3, target_pos: Vec3) -> WeaponEffect {
        WeaponEffect::tank_cannon(muzzle_pos, target_pos)
    }

    pub fn create_machine_gun_effect(muzzle_pos: Vec3) -> WeaponEffect {
        WeaponEffect::machine_gun(muzzle_pos)
    }

    pub fn create_missile_effect(muzzle_pos: Vec3, target_pos: Vec3) -> WeaponEffect {
        WeaponEffect::missile_launcher(muzzle_pos, target_pos)
    }
}
