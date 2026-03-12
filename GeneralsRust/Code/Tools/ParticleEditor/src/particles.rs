//! Particle System Module
//!
//! Handles particle system definitions, templates, and core particle logic.
//! Based on GeneralsMD/Code/GameEngine/Include/GameClient/ParticleSys.h

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Core types from the C++ version
pub type Real = f32;
pub type UnsignedInt = u32;
pub type Int = i32;
pub type Bool = bool;
pub type Byte = u8;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Coord3D {
    pub x: Real,
    pub y: Real,
    pub z: Real,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RGBColor {
    pub red: Real,
    pub green: Real,
    pub blue: Real,
}

// Keyframe structures
pub const MAX_KEYFRAMES: usize = 8;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Keyframe {
    pub value: Real,
    pub frame: UnsignedInt,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RGBColorKeyframe {
    pub color: RGBColor,
    pub frame: UnsignedInt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RandomKeyframe {
    pub var: GameClientRandomVariable,
    pub frame: UnsignedInt,
}

// Random variable system (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameClientRandomVariable {
    pub low: Real,
    pub high: Real,
    pub distribution: DistributionType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DistributionType {
    Uniform,
    Normal,
    Constant,
}

// Priority types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticlePriorityType {
    Invalid = 0,
    WeaponExplosion = 1,
    ScorchMark,
    DustTrail,
    Buildup,
    DebrisTrail,
    UnitDamageFx,
    DeathExplosion,
    SemiConstant,
    Constant,
    WeaponTrail,
    AreaEffect,
    Critical,
    AlwaysRender,
}

// Shader types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticleShaderType {
    Invalid = 0,
    Additive,
    Alpha,
    AlphaTest,
    Multiply,
}

// Particle types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticleType {
    Invalid = 0,
    Particle,
    Drawable,
    Streak,
    VolumeParticle,
    Smudge,
}

// Emission velocity types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmissionVelocityType {
    Invalid = 0,
    Ortho,
    Spherical,
    Hemispherical,
    Cylindrical,
    Outward,
}

// Emission volume types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmissionVolumeType {
    Invalid = 0,
    Point,
    Line,
    Box,
    Sphere,
    Cylinder,
}

// Wind motion types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindMotion {
    Invalid = 0,
    NotUsed,
    PingPong,
    Circular,
}

// Switch types (from C++ enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwitchType {
    Hollow = 0,
    OneShot,
    AlignXY,
    EmitAboveGroundOnly,
    ParticleUpTowardsEmitter,
}

// Particle info structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticleInfo {
    pub vel: Coord3D,
    pub pos: Coord3D,
    pub emitter_pos: Coord3D,
    pub vel_damping: Real,

    pub angle_z: Real,
    pub angular_rate_z: Real,
    pub angular_damping: Real,

    pub lifetime: UnsignedInt,

    pub size: Real,
    pub size_rate: Real,
    pub size_rate_damping: Real,

    pub alpha_key: [Keyframe; MAX_KEYFRAMES],
    pub color_key: [RGBColorKeyframe; MAX_KEYFRAMES],

    pub color_scale: Real,
    pub wind_randomness: Real,
    pub particle_up_towards_emitter: Bool,
}

// Particle system info (base class in C++)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticleSystemInfo {
    pub name: String,

    // System properties
    pub is_one_shot: Bool,
    pub shader_type: ParticleShaderType,
    pub particle_type: ParticleType,
    pub particle_type_name: String,

    // Random variables
    pub angle_z: GameClientRandomVariable,
    pub angular_rate_z: GameClientRandomVariable,
    pub angular_damping: GameClientRandomVariable,
    pub vel_damping: GameClientRandomVariable,
    pub lifetime: GameClientRandomVariable,

    pub system_lifetime: UnsignedInt,

    pub start_size: GameClientRandomVariable,
    pub start_size_rate: GameClientRandomVariable,
    pub size_rate: GameClientRandomVariable,
    pub size_rate_damping: GameClientRandomVariable,

    pub volume_particle_depth: UnsignedInt,

    // Keyframes
    pub alpha_key: [RandomKeyframe; MAX_KEYFRAMES],
    pub color_key: [RGBColorKeyframe; MAX_KEYFRAMES],

    pub color_scale: GameClientRandomVariable,

    pub burst_delay: GameClientRandomVariable,
    pub burst_count: GameClientRandomVariable,
    pub initial_delay: GameClientRandomVariable,

    // Physics
    pub drift_velocity: Coord3D,
    pub gravity: Real,

    // Slave systems
    pub slave_system_name: String,
    pub slave_pos_offset: Coord3D,
    pub attached_system_name: String,

    // Emission properties
    pub emission_velocity_type: EmissionVelocityType,
    pub priority: ParticlePriorityType,

    // Emission velocity union (simplified as enum variants)
    pub emission_velocity: EmissionVelocityData,

    // Emission volume
    pub emission_volume_type: EmissionVolumeType,
    pub emission_volume: EmissionVolumeData,

    // Flags
    pub is_emission_volume_hollow: Bool,
    pub is_ground_aligned: Bool,
    pub is_emit_above_ground_only: Bool,
    pub is_particle_up_towards_emitter: Bool,

    // Wind
    pub wind_motion: WindMotion,
    pub wind_angle: Real,
    pub wind_angle_change: Real,
    pub wind_angle_change_min: Real,
    pub wind_angle_change_max: Real,
    pub wind_motion_start_angle: Real,
    pub wind_motion_start_angle_min: Real,
    pub wind_motion_start_angle_max: Real,
    pub wind_motion_end_angle: Real,
    pub wind_motion_end_angle_min: Real,
    pub wind_motion_end_angle_max: Real,
    pub wind_motion_moving_to_end_angle: Bool,
}

// Emission velocity data (union in C++)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmissionVelocityData {
    Ortho {
        x: GameClientRandomVariable,
        y: GameClientRandomVariable,
        z: GameClientRandomVariable,
    },
    Spherical {
        speed: GameClientRandomVariable,
    },
    Hemispherical {
        speed: GameClientRandomVariable,
    },
    Cylindrical {
        radial: GameClientRandomVariable,
        normal: GameClientRandomVariable,
    },
    Outward {
        speed: GameClientRandomVariable,
        other_speed: GameClientRandomVariable,
    },
}

// Emission volume data (union in C++)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmissionVolumeData {
    Point,
    Line { start: Coord3D, end: Coord3D },
    Box { half_size: Coord3D },
    Sphere { radius: Real },
    Cylinder { radius: Real, length: Real },
}

// Main particle system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticleSystem {
    pub info: ParticleSystemInfo,
    pub particles: Vec<Particle>,
    pub is_active: Bool,
    pub current_time: Real,
}

impl ParticleSystem {
    pub fn new(name: String) -> Result<Self> {
        Ok(Self {
            info: ParticleSystemInfo::new(name),
            particles: Vec::new(),
            is_active: false,
            current_time: 0.0,
        })
    }

    pub fn from_template(template: &ParticleSystemTemplate) -> Result<Self> {
        Ok(Self {
            info: template.info.clone(),
            particles: Vec::new(),
            is_active: false,
            current_time: 0.0,
        })
    }

    pub fn load(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let system: Self = serde_json::from_str(&content)?;
        Ok(system)
    }

    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn reset(&mut self) {
        self.particles.clear();
        self.current_time = 0.0;
        self.is_active = false;
    }

    pub fn update(&mut self, dt: Real) -> Result<()> {
        self.current_time += dt;
        self.is_active = true;

        // Update existing particles
        self.particles.retain_mut(|particle| {
            // Update particle position
            particle.info.pos.x += particle.info.vel.x * dt;
            particle.info.pos.y += particle.info.vel.y * dt;
            particle.info.pos.z += particle.info.vel.z * dt;

            // Apply damping
            particle.info.vel.x *= 1.0 - particle.info.vel_damping * dt;
            particle.info.vel.y *= 1.0 - particle.info.vel_damping * dt;
            particle.info.vel.z *= 1.0 - particle.info.vel_damping * dt;

            // Apply gravity
            particle.info.vel.z += self.info.gravity * dt;

            // Update rotation
            particle.info.angle_z += particle.info.angular_rate_z * dt;
            particle.info.angular_rate_z *= 1.0 - particle.info.angular_damping * dt;

            // Update size
            particle.info.size += particle.info.size_rate * dt;
            particle.info.size_rate *= 1.0 - particle.info.size_rate_damping * dt;

            // Decrement lifetime (lifetime is in frames, we need to convert dt to frames)
            // Assuming 30 fps for particle lifetime
            let frame_dt = (dt * 30.0) as UnsignedInt;
            if particle.info.lifetime > frame_dt {
                particle.info.lifetime -= frame_dt;
                true // Keep particle alive
            } else {
                false // Remove particle
            }
        });

        // Emit new particles based on burst count and delay
        if self.current_time >= self.info.initial_delay.get_value() {
            let burst_interval = self.info.burst_delay.get_value();
            let frames_since_start =
                (self.current_time - self.info.initial_delay.get_value()) * 30.0;
            let burst_count = ((frames_since_start / (burst_interval * 30.0)).floor() as usize + 1)
                * self.info.burst_count.get_value() as usize;

            // Emit particles if we haven't reached the desired count yet
            while self.particles.len() < burst_count.min(1000) {
                self.emit_particle();
            }
        }

        Ok(())
    }

    pub fn seek_to(&mut self, time: Real) {
        self.reset();
        self.current_time = time;
        // Simulate up to the target time
        // For simplicity, we'll just set the time without simulating
        // A more sophisticated implementation would simulate in small steps
    }

    pub fn name(&self) -> &str {
        &self.info.name
    }

    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }

    fn emit_particle(&mut self) {
        // Create alpha keyframes from random keyframes
        let mut alpha_key = [Keyframe {
            value: 1.0,
            frame: 0,
        }; MAX_KEYFRAMES];
        for i in 0..MAX_KEYFRAMES {
            alpha_key[i] = Keyframe {
                value: self.info.alpha_key[i].var.get_value(),
                frame: self.info.alpha_key[i].frame,
            };
        }

        let particle_info = ParticleInfo {
            vel: self.calculate_initial_velocity(),
            pos: self.calculate_emission_position(),
            emitter_pos: Coord3D::default(),
            vel_damping: self.info.vel_damping.get_value(),
            angle_z: self.info.angle_z.get_value(),
            angular_rate_z: self.info.angular_rate_z.get_value(),
            angular_damping: self.info.angular_damping.get_value(),
            lifetime: self.info.lifetime.get_value() as UnsignedInt,
            size: self.info.start_size.get_value(),
            size_rate: self.info.size_rate.get_value(),
            size_rate_damping: self.info.size_rate_damping.get_value(),
            alpha_key,
            color_key: self.info.color_key,
            color_scale: self.info.color_scale.get_value(),
            wind_randomness: 0.0,
            particle_up_towards_emitter: self.info.is_particle_up_towards_emitter,
        };

        let particle = Particle::new(self, &particle_info);
        self.particles.push(particle);
    }

    fn calculate_initial_velocity(&self) -> Coord3D {
        match &self.info.emission_velocity {
            EmissionVelocityData::Ortho { x, y, z } => Coord3D {
                x: x.get_value(),
                y: y.get_value(),
                z: z.get_value(),
            },
            EmissionVelocityData::Spherical { speed } => {
                let speed_val = speed.get_value();
                // Simplified spherical emission (should use proper random angles)
                Coord3D {
                    x: speed_val * 0.5,
                    y: speed_val * 0.5,
                    z: speed_val * 0.7,
                }
            }
            EmissionVelocityData::Hemispherical { speed } => {
                let speed_val = speed.get_value();
                // Simplified hemispherical emission (upward hemisphere)
                Coord3D {
                    x: speed_val * 0.5,
                    y: speed_val * 0.5,
                    z: speed_val.abs(),
                }
            }
            EmissionVelocityData::Cylindrical { radial, normal } => Coord3D {
                x: radial.get_value(),
                y: 0.0,
                z: normal.get_value(),
            },
            EmissionVelocityData::Outward { speed, other_speed } => Coord3D {
                x: speed.get_value(),
                y: other_speed.get_value(),
                z: 0.0,
            },
        }
    }

    fn calculate_emission_position(&self) -> Coord3D {
        match &self.info.emission_volume {
            EmissionVolumeData::Point => Coord3D::default(),
            EmissionVolumeData::Line { start, end } => {
                // Interpolate between start and end (simplified - should be random)
                let t = 0.5;
                Coord3D {
                    x: start.x + (end.x - start.x) * t,
                    y: start.y + (end.y - start.y) * t,
                    z: start.z + (end.z - start.z) * t,
                }
            }
            EmissionVolumeData::Box { half_size } => {
                // Random position within box (simplified)
                Coord3D {
                    x: half_size.x * 0.5,
                    y: half_size.y * 0.5,
                    z: half_size.z * 0.5,
                }
            }
            EmissionVolumeData::Sphere { radius } => {
                // Random position on/in sphere (simplified)
                Coord3D {
                    x: radius * 0.5,
                    y: radius * 0.5,
                    z: radius * 0.5,
                }
            }
            EmissionVolumeData::Cylinder { radius, length } => {
                // Random position in cylinder (simplified)
                Coord3D {
                    x: radius * 0.5,
                    y: radius * 0.5,
                    z: length * 0.5,
                }
            }
        }
    }
}

// Individual particle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Particle {
    pub info: ParticleInfo,
    pub current_alpha: Real,
    pub current_color: RGBColor,
    pub is_culled: Bool,
    pub personality: UnsignedInt,
}

impl Particle {
    pub fn new(system: &ParticleSystem, data: &ParticleInfo) -> Self {
        Self {
            info: data.clone(),
            current_alpha: 1.0,
            current_color: RGBColor {
                red: 1.0,
                green: 1.0,
                blue: 1.0,
            },
            is_culled: false,
            personality: 0,
        }
    }
}

// Particle system template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticleSystemTemplate {
    pub info: ParticleSystemInfo,
}

impl ParticleSystemTemplate {
    pub fn new(name: String) -> Self {
        Self {
            info: ParticleSystemInfo::new(name),
        }
    }

    pub fn get_name(&self) -> &str {
        &self.info.name
    }

    pub fn fire() -> Self {
        let mut template = Self::new("Fire".to_string());
        template.info.shader_type = ParticleShaderType::Additive;
        template.info.lifetime = GameClientRandomVariable::new(20.0, 40.0);
        template.info.start_size = GameClientRandomVariable::new(0.5, 1.0);
        template.info.size_rate = GameClientRandomVariable::new(1.0, 2.0);
        template.info.emission_velocity = EmissionVelocityData::Spherical {
            speed: GameClientRandomVariable::new(0.5, 2.0),
        };
        template.info.gravity = -2.0;
        template.info.color_key[0] = RGBColorKeyframe {
            color: RGBColor {
                red: 1.0,
                green: 0.5,
                blue: 0.0,
            },
            frame: 0,
        };
        template.info.color_key[1] = RGBColorKeyframe {
            color: RGBColor {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
            },
            frame: 15,
        };
        template
    }

    pub fn smoke() -> Self {
        let mut template = Self::new("Smoke".to_string());
        template.info.shader_type = ParticleShaderType::Alpha;
        template.info.lifetime = GameClientRandomVariable::new(40.0, 60.0);
        template.info.start_size = GameClientRandomVariable::new(1.0, 2.0);
        template.info.size_rate = GameClientRandomVariable::new(2.0, 4.0);
        template.info.emission_velocity = EmissionVelocityData::Hemispherical {
            speed: GameClientRandomVariable::new(1.0, 3.0),
        };
        template.info.gravity = -1.0;
        template.info.vel_damping = GameClientRandomVariable::constant(0.5);
        template.info.color_key[0] = RGBColorKeyframe {
            color: RGBColor {
                red: 0.3,
                green: 0.3,
                blue: 0.3,
            },
            frame: 0,
        };
        template.info.color_key[1] = RGBColorKeyframe {
            color: RGBColor {
                red: 0.5,
                green: 0.5,
                blue: 0.5,
            },
            frame: 30,
        };
        template
    }

    pub fn explosion() -> Self {
        let mut template = Self::new("Explosion".to_string());
        template.info.shader_type = ParticleShaderType::Additive;
        template.info.is_one_shot = true;
        template.info.lifetime = GameClientRandomVariable::new(10.0, 20.0);
        template.info.start_size = GameClientRandomVariable::new(1.0, 2.0);
        template.info.size_rate = GameClientRandomVariable::new(5.0, 10.0);
        template.info.emission_velocity = EmissionVelocityData::Spherical {
            speed: GameClientRandomVariable::new(5.0, 15.0),
        };
        template.info.emission_volume = EmissionVolumeData::Sphere { radius: 0.5 };
        template.info.burst_count = GameClientRandomVariable::constant(50.0);
        template.info.color_key[0] = RGBColorKeyframe {
            color: RGBColor {
                red: 1.0,
                green: 1.0,
                blue: 0.5,
            },
            frame: 0,
        };
        template.info.color_key[1] = RGBColorKeyframe {
            color: RGBColor {
                red: 1.0,
                green: 0.3,
                blue: 0.0,
            },
            frame: 10,
        };
        template
    }

    pub fn sparks() -> Self {
        let mut template = Self::new("Sparks".to_string());
        template.info.shader_type = ParticleShaderType::Additive;
        template.info.particle_type = ParticleType::Streak;
        template.info.lifetime = GameClientRandomVariable::new(5.0, 15.0);
        template.info.start_size = GameClientRandomVariable::new(0.1, 0.3);
        template.info.emission_velocity = EmissionVelocityData::Spherical {
            speed: GameClientRandomVariable::new(3.0, 10.0),
        };
        template.info.gravity = 9.8;
        template.info.burst_count = GameClientRandomVariable::constant(20.0);
        template.info.color_key[0] = RGBColorKeyframe {
            color: RGBColor {
                red: 1.0,
                green: 1.0,
                blue: 0.5,
            },
            frame: 0,
        };
        template.info.color_key[1] = RGBColorKeyframe {
            color: RGBColor {
                red: 1.0,
                green: 0.5,
                blue: 0.0,
            },
            frame: 10,
        };
        template
    }

    pub fn magic() -> Self {
        let mut template = Self::new("Magic".to_string());
        template.info.shader_type = ParticleShaderType::Additive;
        template.info.lifetime = GameClientRandomVariable::new(30.0, 50.0);
        template.info.start_size = GameClientRandomVariable::new(0.3, 0.8);
        template.info.size_rate = GameClientRandomVariable::new(0.5, 1.5);
        template.info.emission_velocity = EmissionVelocityData::Cylindrical {
            radial: GameClientRandomVariable::new(0.5, 1.5),
            normal: GameClientRandomVariable::new(2.0, 4.0),
        };
        template.info.emission_volume = EmissionVolumeData::Cylinder {
            radius: 0.5,
            length: 1.0,
        };
        template.info.gravity = -1.5;
        template.info.angular_rate_z = GameClientRandomVariable::new(-3.0, 3.0);
        template.info.color_key[0] = RGBColorKeyframe {
            color: RGBColor {
                red: 0.5,
                green: 0.0,
                blue: 1.0,
            },
            frame: 0,
        };
        template.info.color_key[1] = RGBColorKeyframe {
            color: RGBColor {
                red: 0.0,
                green: 0.5,
                blue: 1.0,
            },
            frame: 25,
        };
        template
    }

    pub fn water() -> Self {
        let mut template = Self::new("Water".to_string());
        template.info.shader_type = ParticleShaderType::Alpha;
        template.info.lifetime = GameClientRandomVariable::new(20.0, 40.0);
        template.info.start_size = GameClientRandomVariable::new(0.3, 0.6);
        template.info.emission_velocity = EmissionVelocityData::Hemispherical {
            speed: GameClientRandomVariable::new(2.0, 5.0),
        };
        template.info.gravity = 15.0;
        template.info.vel_damping = GameClientRandomVariable::constant(0.1);
        template.info.color_key[0] = RGBColorKeyframe {
            color: RGBColor {
                red: 0.3,
                green: 0.5,
                blue: 1.0,
            },
            frame: 0,
        };
        template.info.color_key[1] = RGBColorKeyframe {
            color: RGBColor {
                red: 0.2,
                green: 0.4,
                blue: 0.8,
            },
            frame: 20,
        };
        template
    }
}

// Implementation of ParticleSystemInfo
impl ParticleSystemInfo {
    pub fn new(name: String) -> Self {
        Self {
            name,
            is_one_shot: false,
            shader_type: ParticleShaderType::Additive,
            particle_type: ParticleType::Particle,
            particle_type_name: String::new(),

            angle_z: GameClientRandomVariable::constant(0.0),
            angular_rate_z: GameClientRandomVariable::constant(0.0),
            angular_damping: GameClientRandomVariable::constant(0.0),
            vel_damping: GameClientRandomVariable::constant(0.0),
            lifetime: GameClientRandomVariable::constant(30.0),

            system_lifetime: 0,

            start_size: GameClientRandomVariable::constant(1.0),
            start_size_rate: GameClientRandomVariable::constant(0.0),
            size_rate: GameClientRandomVariable::constant(0.0),
            size_rate_damping: GameClientRandomVariable::constant(0.0),

            volume_particle_depth: 0,

            alpha_key: std::array::from_fn(|_| RandomKeyframe {
                var: GameClientRandomVariable::constant(1.0),
                frame: 0,
            }),

            color_key: [RGBColorKeyframe {
                color: RGBColor {
                    red: 1.0,
                    green: 1.0,
                    blue: 1.0,
                },
                frame: 0,
            }; MAX_KEYFRAMES],

            color_scale: GameClientRandomVariable::constant(1.0),

            burst_delay: GameClientRandomVariable::constant(1.0),
            burst_count: GameClientRandomVariable::constant(1.0),
            initial_delay: GameClientRandomVariable::constant(0.0),

            drift_velocity: Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            gravity: 0.0,

            slave_system_name: String::new(),
            slave_pos_offset: Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            attached_system_name: String::new(),

            emission_velocity_type: EmissionVelocityType::Ortho,
            priority: ParticlePriorityType::WeaponExplosion,

            emission_velocity: EmissionVelocityData::Ortho {
                x: GameClientRandomVariable::constant(0.0),
                y: GameClientRandomVariable::constant(0.0),
                z: GameClientRandomVariable::constant(1.0),
            },

            emission_volume_type: EmissionVolumeType::Point,
            emission_volume: EmissionVolumeData::Point,

            is_emission_volume_hollow: false,
            is_ground_aligned: false,
            is_emit_above_ground_only: false,
            is_particle_up_towards_emitter: false,

            wind_motion: WindMotion::NotUsed,
            wind_angle: 0.0,
            wind_angle_change: 0.0,
            wind_angle_change_min: 0.0,
            wind_angle_change_max: 0.0,
            wind_motion_start_angle: 0.0,
            wind_motion_start_angle_min: 0.0,
            wind_motion_start_angle_max: 0.0,
            wind_motion_end_angle: 0.0,
            wind_motion_end_angle_min: 0.0,
            wind_motion_end_angle_max: 0.0,
            wind_motion_moving_to_end_angle: false,
        }
    }
}

// Implementation of GameClientRandomVariable
impl GameClientRandomVariable {
    pub fn new(low: Real, high: Real) -> Self {
        Self {
            low,
            high,
            distribution: DistributionType::Uniform,
        }
    }

    pub fn constant(value: Real) -> Self {
        Self {
            low: value,
            high: value,
            distribution: DistributionType::Constant,
        }
    }

    pub fn get_value(&self) -> Real {
        match self.distribution {
            DistributionType::Constant => self.low,
            DistributionType::Uniform => {
                // Simple random for now - in real implementation would use proper RNG
                self.low + (self.high - self.low) * 0.5
            }
            DistributionType::Normal => {
                // Simplified normal distribution
                self.low + (self.high - self.low) * 0.5
            }
        }
    }
}

// Default implementations
impl Default for Coord3D {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

impl Default for RGBColor {
    fn default() -> Self {
        Self {
            red: 1.0,
            green: 1.0,
            blue: 1.0,
        }
    }
}
