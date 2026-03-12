//! Particle Loader Implementation
//!
//! This module implements loading particle systems from W3D files,
//! corresponding to the part_ldr.h/cpp functionality in the C++ version.

use super::buffer::{FrameMode, RenderMode};
use super::emitter::ParticleEmitter;
use super::properties::*;
use binrw::{BinRead, BinReaderExt};
use glam::Vec3;
use std::io::{Read, Seek};

/// W3D particle emitter definition structure
#[derive(Debug, Clone, BinRead, binrw::BinWrite)]
#[brw(little)]
pub struct W3dParticleEmitterStruct {
    pub version: u32,
    pub name: [u8; 16], // Fixed-size string
    pub emitter_props: W3dEmitterPropertyStruct,
    pub particle_props: W3dParticlePropertyStruct,
}

#[derive(Debug, Clone, BinRead, binrw::BinWrite)]
#[brw(little)]
pub struct W3dEmitterPropertyStruct {
    pub color_keyframes: u32,
    pub opacity_keyframes: u32,
    pub size_keyframes: u32,
    pub velocity_keyframes: u32,
    pub rotation_keyframes: u32,
    pub frame_keyframes: u32,
    pub blur_time_keyframes: u32,
    pub lifetime: f32,
    pub emission_rate: f32,
    pub max_emissions: u32,
    pub velocity: [f32; 3],
    pub acceleration: [f32; 3],
    pub min_tint: [f32; 3],
    pub max_tint: [f32; 3],
    pub start_size: f32,
    pub end_size: f32,
    pub mass: f32,
    pub bounce: f32,
    pub friction: f32,
    pub elasticity: f32,
    pub wind_influence: f32,
    pub particle_up_vector: [f32; 3],
    pub position_random: [f32; 3],
    pub velocity_random: [f32; 3],
    pub position_random_is_local: u32,
    pub velocity_random_is_local: u32,
    pub gravity: f32,
    pub particle_lifetime: f32,
    pub texture_checksum: u32,
    pub shader_checksum: u32,
}

#[derive(Debug, Clone, BinRead, binrw::BinWrite)]
#[brw(little)]
pub struct W3dParticlePropertyStruct {
    pub color: [f32; 3],
    pub opacity: f32,
    pub size: f32,
    pub velocity: [f32; 3],
    pub rotation: f32,
    pub frame: f32,
    pub blur_time: f32,
}

/// Particle loader for loading particle systems from W3D files
pub struct ParticleLoader;

impl ParticleLoader {
    /// Load a particle emitter from a W3D file
    pub fn load_emitter_from_w3d<R: Read + Seek>(
        reader: &mut R,
        name: &str,
    ) -> Result<ParticleEmitter, Box<dyn std::error::Error>> {
        // Read the emitter header
        let emitter_struct: W3dParticleEmitterStruct = BinRead::read(reader)?;

        // Convert name from fixed-size array to string
        let emitter_name = Self::fixed_array_to_string(&emitter_struct.name);

        // Create particle properties from the loaded data
        let color_prop =
            ParticleColorProperty::with_start(Vec3::from(emitter_struct.particle_props.color));
        let opacity_prop =
            ParticleOpacityProperty::with_start(emitter_struct.particle_props.opacity);
        let size_prop = ParticleSizeProperty::with_start(emitter_struct.particle_props.size);
        let rotation_prop =
            ParticleRotationProperty::with_start(emitter_struct.particle_props.rotation);
        let frame_prop = ParticleFrameProperty::with_start(emitter_struct.particle_props.frame);
        let blur_time_prop =
            ParticleBlurTimeProperty::with_start(emitter_struct.particle_props.blur_time);

        // Create the emitter
        let mut emitter = ParticleEmitter::new(
            emitter_struct.emitter_props.emission_rate,
            1,    // burst_size - could be derived from other properties
            None, // position_randomizer
            Vec3::from(emitter_struct.emitter_props.velocity),
            None, // velocity_randomizer
            0.0,  // outward_velocity
            0.0,  // velocity_inherit_factor
            color_prop,
            opacity_prop,
            size_prop,
            rotation_prop,
            0.0, // orient_random
            frame_prop,
            blur_time_prop,
            Vec3::from(emitter_struct.emitter_props.acceleration),
            emitter_struct.emitter_props.lifetime,
            0.0,                      // future_start
            RenderMode::TriParticles, // default render mode
            FrameMode::Frame1x1,      // default frame mode
            emitter_struct.emitter_props.max_emissions as usize,
            1000,  // max_buffer_size - could be configurable
            false, // pingpong
        );

        // Set the emitter name
        emitter.name = if name.is_empty() {
            emitter_name
        } else {
            name.to_string()
        };

        Ok(emitter)
    }

    /// Load particle properties from keyframe data
    pub fn load_keyframes<R: Read + Seek>(
        reader: &mut R,
        num_color_frames: u32,
        num_opacity_frames: u32,
        num_size_frames: u32,
        num_rotation_frames: u32,
        num_frame_frames: u32,
        num_blur_frames: u32,
    ) -> Result<
        (
            ParticleColorProperty,
            ParticleOpacityProperty,
            ParticleSizeProperty,
            ParticleRotationProperty,
            ParticleFrameProperty,
            ParticleBlurTimeProperty,
        ),
        Box<dyn std::error::Error>,
    > {
        // Load color keyframes
        let color_prop = if num_color_frames > 1 {
            let times = Self::read_f32_array(reader, num_color_frames)?;
            let values = Self::read_vec3_array(reader, num_color_frames)?;
            ParticleColorProperty::with_keyframes(Vec3::ONE, Vec3::ZERO, times, values)
        } else {
            let color_arr: [f32; 3] = reader.read_le()?;
            let color = Vec3::from(color_arr);
            ParticleColorProperty::with_start(color)
        };

        // Load opacity keyframes
        let opacity_prop = if num_opacity_frames > 1 {
            let times = Self::read_f32_array(reader, num_opacity_frames)?;
            let values = Self::read_f32_array(reader, num_opacity_frames)?;
            ParticleOpacityProperty::with_keyframes(1.0, 0.0, times, values)
        } else {
            let opacity = reader.read_le::<f32>()?;
            ParticleOpacityProperty::with_start(opacity)
        };

        // Load size keyframes
        let size_prop = if num_size_frames > 1 {
            let times = Self::read_f32_array(reader, num_size_frames)?;
            let values = Self::read_f32_array(reader, num_size_frames)?;
            ParticleSizeProperty::with_keyframes(1.0, 0.0, times, values)
        } else {
            let size = reader.read_le::<f32>()?;
            ParticleSizeProperty::with_start(size)
        };

        // Load rotation keyframes
        let rotation_prop = if num_rotation_frames > 1 {
            let times = Self::read_f32_array(reader, num_rotation_frames)?;
            let values = Self::read_f32_array(reader, num_rotation_frames)?;
            ParticleRotationProperty::with_keyframes(0.0, 0.0, 0.0, times, values)
        } else {
            let rotation = reader.read_le::<f32>()?;
            ParticleRotationProperty::with_start(rotation)
        };

        // Load frame keyframes
        let frame_prop = if num_frame_frames > 1 {
            let times = Self::read_f32_array(reader, num_frame_frames)?;
            let values = Self::read_f32_array(reader, num_frame_frames)?;
            ParticleFrameProperty::with_keyframes(0.0, 0.0, times, values)
        } else {
            let frame = reader.read_le::<f32>()?;
            ParticleFrameProperty::with_start(frame)
        };

        // Load blur time keyframes
        let blur_time_prop = if num_blur_frames > 1 {
            let times = Self::read_f32_array(reader, num_blur_frames)?;
            let values = Self::read_f32_array(reader, num_blur_frames)?;
            ParticleBlurTimeProperty::with_keyframes(0.0, 0.0, times, values)
        } else {
            let blur_time = reader.read_le::<f32>()?;
            ParticleBlurTimeProperty::with_start(blur_time)
        };

        Ok((
            color_prop,
            opacity_prop,
            size_prop,
            rotation_prop,
            frame_prop,
            blur_time_prop,
        ))
    }

    /// Helper function to read an array of f32 values
    fn read_f32_array<R: Read + Seek>(
        reader: &mut R,
        count: u32,
    ) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let mut values = Vec::with_capacity(count as usize);
        for _ in 0..count {
            values.push(reader.read_le::<f32>()?);
        }
        Ok(values)
    }

    /// Helper function to read an array of Vec3 values
    fn read_vec3_array<R: Read + Seek>(
        reader: &mut R,
        count: u32,
    ) -> Result<Vec<Vec3>, Box<dyn std::error::Error>> {
        let mut values = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let arr: [f32; 3] = reader.read_le()?;
            values.push(Vec3::from(arr));
        }
        Ok(values)
    }

    /// Convert fixed-size byte array to string
    fn fixed_array_to_string(bytes: &[u8; 16]) -> String {
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(16);
        String::from_utf8_lossy(&bytes[..end]).to_string()
    }

    /// Create a simple particle emitter for testing
    pub fn create_simple_emitter() -> ParticleEmitter {
        let color_prop = ParticleColorProperty::with_start(Vec3::new(1.0, 0.5, 0.0)); // Orange
        let opacity_prop = ParticleOpacityProperty::with_start(1.0);
        let size_prop = ParticleSizeProperty::with_start(1.0);
        let rotation_prop = ParticleRotationProperty::with_start(0.0);
        let frame_prop = ParticleFrameProperty::with_start(0.0);
        let blur_time_prop = ParticleBlurTimeProperty::with_start(0.0);

        ParticleEmitter::new(
            100.0,                    // 100 particles per second
            1,                        // burst size
            None,                     // position randomizer
            Vec3::new(0.0, 1.0, 0.0), // velocity up
            None,                     // velocity randomizer
            0.0,                      // outward velocity
            0.0,                      // velocity inherit factor
            color_prop,
            opacity_prop,
            size_prop,
            rotation_prop,
            0.0, // orient random
            frame_prop,
            blur_time_prop,
            Vec3::new(0.0, -9.8, 0.0), // gravity
            2.0,                       // lifetime
            0.0,                       // future start
            RenderMode::TriParticles,
            FrameMode::Frame1x1,
            1000,  // max particles
            1000,  // max buffer size
            false, // pingpong
        )
    }

    /// Create a fire particle emitter
    pub fn create_fire_emitter() -> ParticleEmitter {
        let color_prop = ParticleColorProperty::with_keyframes(
            Vec3::new(1.0, 0.3, 0.0), // Orange start
            Vec3::new(0.2, 0.2, 0.2), // Random variation
            vec![0.0, 0.5, 1.0],      // Times
            vec![
                Vec3::new(1.0, 0.3, 0.0), // Start: Orange
                Vec3::new(1.0, 0.6, 0.0), // Middle: Yellow-orange
                Vec3::new(0.5, 0.5, 0.5), // End: Gray
            ],
        );

        let opacity_prop = ParticleOpacityProperty::with_keyframes(
            1.0,
            0.0,
            vec![0.0, 0.7, 1.0],
            vec![1.0, 0.8, 0.0],
        );

        let size_prop = ParticleSizeProperty::with_keyframes(
            0.5,
            0.2,
            vec![0.0, 0.5, 1.0],
            vec![0.5, 1.0, 0.1],
        );

        let rotation_prop = ParticleRotationProperty::with_start(0.0);
        let frame_prop = ParticleFrameProperty::with_start(0.0);
        let blur_time_prop = ParticleBlurTimeProperty::with_start(0.0);

        ParticleEmitter::new(
            50.0, // 50 particles per second
            2,    // burst size
            Some(super::emitter::Vec3Randomizer::new(
                Vec3::new(-0.5, -0.5, -0.5),
                Vec3::new(0.5, 0.5, 0.5),
            )), // position randomizer
            Vec3::new(0.0, 2.0, 0.0), // velocity up
            Some(super::emitter::Vec3Randomizer::new(
                Vec3::new(-0.5, -0.5, -0.5),
                Vec3::new(0.5, 0.5, 0.5),
            )), // velocity randomizer
            0.0,  // outward velocity
            0.0,  // velocity inherit factor
            color_prop,
            opacity_prop,
            size_prop,
            rotation_prop,
            0.5, // orient random
            frame_prop,
            blur_time_prop,
            Vec3::new(0.0, -1.0, 0.0), // light gravity
            3.0,                       // lifetime
            0.0,                       // future start
            RenderMode::TriParticles,
            FrameMode::Frame1x1,
            500,   // max particles
            1000,  // max buffer size
            false, // pingpong
        )
    }
}
