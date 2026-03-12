//! Particle Properties System
//!
//! This module handles particle property definitions and configurations.

use glam::{Vec3, Vec4};
use ww3d_core::errors::{W3DError, W3DResult};

/// Particle property types
#[derive(Clone, Debug)]
pub enum PropertyType {
    Color(Vec4),
    Size(f32),
    Speed(f32),
    Lifetime(f32),
    Gravity(Vec3),
    Texture(String),
}

/// Particle property collection
#[derive(Clone, Debug)]
pub struct ParticleProperties {
    properties: Vec<PropertyType>,
}

impl ParticleProperties {
    /// Create new particle properties
    pub fn new() -> Self {
        Self {
            properties: Vec::new(),
        }
    }

    /// Add a property
    pub fn add_property(&mut self, property: PropertyType) {
        self.properties.push(property);
    }

    /// Get property by index
    pub fn get_property(&self, index: usize) -> Option<&PropertyType> {
        self.properties.get(index)
    }

    /// Set property at index
    pub fn set_property(&mut self, index: usize, property: PropertyType) -> W3DResult<()> {
        if index >= self.properties.len() {
            return Err(W3DError::InvalidParameter(
                "Property index out of bounds".to_string(),
            ));
        }
        self.properties[index] = property;
        Ok(())
    }

    /// Get property count
    pub fn property_count(&self) -> usize {
        self.properties.len()
    }

    /// Clear all properties
    pub fn clear(&mut self) {
        self.properties.clear();
    }
}

/// Predefined particle property sets
pub struct ParticlePropertySets;

impl ParticlePropertySets {
    /// Create smoke particle properties
    pub fn smoke() -> ParticleProperties {
        let mut props = ParticleProperties::new();
        props.add_property(PropertyType::Color(Vec4::new(0.5, 0.5, 0.5, 0.3)));
        props.add_property(PropertyType::Size(2.0));
        props.add_property(PropertyType::Speed(5.0));
        props.add_property(PropertyType::Lifetime(3.0));
        props.add_property(PropertyType::Gravity(Vec3::new(0.0, 2.0, 0.0)));
        props
    }

    /// Create fire particle properties
    pub fn fire() -> ParticleProperties {
        let mut props = ParticleProperties::new();
        props.add_property(PropertyType::Color(Vec4::new(1.0, 0.5, 0.0, 1.0)));
        props.add_property(PropertyType::Size(1.5));
        props.add_property(PropertyType::Speed(8.0));
        props.add_property(PropertyType::Lifetime(1.5));
        props.add_property(PropertyType::Gravity(Vec3::new(0.0, 1.0, 0.0)));
        props
    }

    /// Create water particle properties
    pub fn water() -> ParticleProperties {
        let mut props = ParticleProperties::new();
        props.add_property(PropertyType::Color(Vec4::new(0.2, 0.4, 1.0, 0.8)));
        props.add_property(PropertyType::Size(0.5));
        props.add_property(PropertyType::Speed(12.0));
        props.add_property(PropertyType::Lifetime(2.0));
        props.add_property(PropertyType::Gravity(Vec3::new(0.0, -15.0, 0.0)));
        props
    }

    /// Create explosion particle properties
    pub fn explosion() -> ParticleProperties {
        let mut props = ParticleProperties::new();
        props.add_property(PropertyType::Color(Vec4::new(1.0, 0.8, 0.0, 1.0)));
        props.add_property(PropertyType::Size(3.0));
        props.add_property(PropertyType::Speed(20.0));
        props.add_property(PropertyType::Lifetime(0.8));
        props.add_property(PropertyType::Gravity(Vec3::ZERO));
        props
    }
}

/// Particle property loader
pub struct ParticlePropertyLoader;

impl ParticlePropertyLoader {
    /// Load properties from file
    pub fn load_from_file(_filename: &str) -> W3DResult<ParticleProperties> {
        // Placeholder implementation
        Ok(ParticleProperties::new())
    }

    /// Save properties to file
    pub fn save_to_file(_properties: &ParticleProperties, _filename: &str) -> W3DResult<()> {
        // Placeholder implementation
        Ok(())
    }
}
