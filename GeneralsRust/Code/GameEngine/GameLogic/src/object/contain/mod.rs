//! Object Containment System
//!
//! This module provides various container implementations for the Command & Conquer Generals
//! Zero Hour game logic. Each container type has specialized behavior for different gameplay
//! mechanics such as transportation, garrisoning, healing, and special abilities.
//!
//! ## Container Types
//!
//! - **OpenContain**: Base container functionality for all containment systems
//! - **TransportContain**: Standard transport functionality for moving units
//! - **GarrisonContain**: Building garrisoning with combat positioning and healing
//! - **HealContain**: Medical facilities that heal contained units over time
//! - **CaveContain**: Cave network system with distributed containment
//! - **TunnelContain**: Tunnel network transportation system
//! - **HelixContain**: Specialized Helix transport with external payload
//! - **OverlordContain**: Tank-based container with external riders
//! - **ParachuteContain**: Airborne deployment and parachute drops
//! - **RailedTransportContain**: Rail-based transportation system
//! - **RiderChangeContain**: Container that can transform rider types
//! - **InternetHackContain**: Specialized container for hacking functionality
//! - **MobNexusContain**: Mob-based containment system
//!
//! ## Architecture
//!
//! All containers inherit from OpenContain which provides the fundamental containment
//! functionality. Specialized containers override specific methods to implement their
//! unique behaviors while maintaining compatibility with the base system.
//!
//! ## Thread Safety
//!
//! All container modules use thread-safe patterns with Arc<Mutex<T>> for shared data
//! and proper synchronization mechanisms to ensure safe concurrent access in a
//! multi-threaded game environment.

// Re-export all container modules
pub mod cave_contain;
pub mod garrison_contain;
pub mod heal_contain;
pub mod helix_contain;
pub mod internet_hack_contain;
pub mod mob_nexus_contain;
pub mod open_contain;
pub mod overlord_contain;
pub mod parachute_contain;
pub mod railed_transport_contain;
pub mod rider_change_contain;
pub mod transport_contain;
pub mod tunnel_contain;

// Re-export main types for easy access
pub use cave_contain::{CaveContain, CaveContainModuleData};
pub use garrison_contain::{
    EvacDisposition, FirePortAngle, GarrisonContain, GarrisonContainModuleData,
    GarrisonPointCondition, GarrisonPointData, InitialRoster, StationPointData,
};
pub use heal_contain::{HealContain, HealContainModuleData};
pub use helix_contain::{HelixContain, HelixContainModuleData};
pub use internet_hack_contain::{InternetHackContain, InternetHackContainModuleData};
pub use mob_nexus_contain::{MobNexusContain, MobNexusContainModuleData};
pub use open_contain::{ObjectTemplate, OpenContain, OpenContainModuleData, CONTAIN_MAX_UNKNOWN};
pub use overlord_contain::{OverlordContain, OverlordContainModuleData};
pub use parachute_contain::{ParachuteContain, ParachuteContainModuleData};
pub use railed_transport_contain::{RailedTransportContain, RailedTransportContainModuleData};
pub use rider_change_contain::{RiderChangeContain, RiderChangeContainModuleData};
pub use transport_contain::{InitialPayload, TransportContain, TransportContainModuleData};
pub use tunnel_contain::{TunnelContain, TunnelContainModuleData};

use crate::common::GameResult;
use crate::object::{Object, ObjectId};
use game_engine::common::ini::{FieldParse, INIError, INI};
use log::warn;
use serde_json::Value;
use std::sync::{Arc, Mutex, RwLock, Weak};

/// Trait for common container functionality
pub trait ContainerInterface {
    /// Check if this container can contain the given object
    fn can_contain(&self, obj: &Object) -> bool;

    /// Add object to container
    fn add_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()>;

    /// Remove object from container
    fn remove_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()>;

    /// Get current capacity usage
    fn get_usage(&self) -> (u32, u32); // (current, max)

    /// Check if container is full
    fn is_full(&self) -> bool {
        let (current, max) = self.get_usage();
        max != u32::MAX && current >= max
    }

    /// Check if container is empty
    fn is_empty(&self) -> bool {
        self.get_usage().0 == 0
    }
}

/// Container factory for creating appropriate container types
pub struct ContainerFactory;

impl ContainerFactory {
    /// Create a container based on type string
    pub fn create_container(
        container_type: &str,
        object: Arc<RwLock<Object>>,
        config: &str, // JSON or INI config
    ) -> GameResult<Box<dyn ContainerInterface>> {
        let weak_object = Arc::downgrade(&object);
        drop(object);

        match container_type {
            "OpenContain" => {
                let data = Self::load_config::<OpenContainModuleData>(config);
                let container = OpenContain::new(weak_object.clone(), &data)?;
                Ok(Box::new(container))
            }
            "TransportContain" => {
                let data = Self::load_config::<TransportContainModuleData>(config);
                let container = TransportContain::new(weak_object.clone(), &data)?;
                Ok(Box::new(container))
            }
            "GarrisonContain" => {
                let data = Self::load_config::<GarrisonContainModuleData>(config);
                let container = GarrisonContain::new(weak_object.clone(), &data)?;
                Ok(Box::new(container))
            }
            "HealContain" => {
                let data = Self::load_config::<HealContainModuleData>(config);
                let container = HealContain::new(weak_object.clone(), &data)?;
                Ok(Box::new(container))
            }
            "CaveContain" => {
                let data = Self::load_config::<CaveContainModuleData>(config);
                let container = CaveContain::new(weak_object.clone(), &data, None)?;
                Ok(Box::new(container))
            }
            "TunnelContain" => {
                let data = Self::load_config::<TunnelContainModuleData>(config);
                let container = TunnelContain::new(weak_object.clone(), &data)?;
                Ok(Box::new(container))
            }
            "HelixContain" => {
                let data = Self::load_config::<HelixContainModuleData>(config);
                let container = HelixContain::new(weak_object.clone(), &data)?;
                Ok(Box::new(container))
            }
            "OverlordContain" => {
                let data = Self::load_config::<OverlordContainModuleData>(config);
                let container = OverlordContain::new(weak_object.clone(), &data)?;
                Ok(Box::new(container))
            }
            "ParachuteContain" => {
                let data = Self::load_config::<ParachuteContainModuleData>(config);
                let container = ParachuteContain::new(weak_object.clone(), &data)?;
                Ok(Box::new(container))
            }
            "RailedTransportContain" => {
                let data = Self::load_config::<RailedTransportContainModuleData>(config);
                let container = RailedTransportContain::new(weak_object.clone(), &data)?;
                Ok(Box::new(container))
            }
            "RiderChangeContain" => {
                let data = Self::load_config::<RiderChangeContainModuleData>(config);
                let container = RiderChangeContain::new(weak_object.clone(), &data)?;
                Ok(Box::new(container))
            }
            "InternetHackContain" => {
                let data = Self::load_config::<InternetHackContainModuleData>(config);
                let container = InternetHackContain::new(weak_object.clone(), &data)?;
                Ok(Box::new(container))
            }
            "MobNexusContain" => {
                let data = Self::load_config::<MobNexusContainModuleData>(config);
                let container = MobNexusContain::new(weak_object.clone(), &data)?;
                Ok(Box::new(container))
            }
            _ => Err(format!("Unknown container type: {}", container_type).into()),
        }
    }

    fn load_config<T: Default + ContainerIniParse>(config: &str) -> T {
        let trimmed = config.trim();
        if trimmed.is_empty() {
            return T::default();
        }

        if is_json_like(trimmed) {
            match normalize_json_config(trimmed) {
                Ok(json_as_inline) => {
                    let normalized = normalize_inline_config(&json_as_inline);
                    if normalized.is_empty() {
                        return T::default();
                    }

                    let mut data = T::default();
                    if let Err(err) = data.parse_from_config(&normalized) {
                        warn!(
                            "Failed to parse JSON container config; using defaults. Error: {}",
                            err
                        );
                        return T::default();
                    }
                    return data;
                }
                Err(err) => {
                    warn!(
                        "Container config JSON parsing failed; using defaults. Error: {}",
                        err
                    );
                    return T::default();
                }
            }
        }

        let normalized = normalize_inline_config(config);
        if normalized.is_empty() {
            return T::default();
        }

        let mut data = T::default();
        if let Err(err) = data.parse_from_config(&normalized) {
            warn!(
                "Failed to parse container config; using defaults. Error: {}",
                err
            );
        }
        data
    }
}

pub trait ContainerIniParse {
    fn parse_from_config(&mut self, config: &str) -> Result<(), INIError>;
}

pub(super) fn parse_with_fields_allow_unknown<T>(
    config: &str,
    data: &mut T,
    fields: &[FieldParse<T>],
) -> Result<(), INIError> {
    let mut ini = INI::new();
    ini.with_inline_source(config, |ini| {
        ini.init_from_ini_with_fields_allow_unknown(data, fields)
    })?;
    Ok(())
}

pub(super) fn parse_duration_frames_real(token: &str) -> Result<f32, INIError> {
    INI::parse_duration_real(token)
}

fn normalize_inline_config(config: &str) -> String {
    let mut lines = Vec::new();

    for line in config.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with(';') || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }

        let mut tokens = trimmed.split_whitespace();
        if let Some(first) = tokens.next() {
            if first.eq_ignore_ascii_case("Behavior") {
                continue;
            }
            if first.eq_ignore_ascii_case("End") {
                continue;
            }
        }

        lines.push(trimmed.to_string());
    }

    if lines.is_empty() {
        return String::new();
    }

    lines.push("End".to_string());
    lines.join("\n")
}

fn is_json_like(config: &str) -> bool {
    let trimmed = config.trim_start();
    matches!(trimmed.chars().next(), Some('{') | Some('['))
}

fn normalize_json_config(config: &str) -> Result<String, String> {
    let parsed: Value = serde_json::from_str(config).map_err(|e| e.to_string())?;
    let object = match parsed {
        Value::Object(map) => map,
        _ => {
            return Err("container config JSON must be an object".to_string());
        }
    };

    let mut lines = Vec::new();
    for (key, value) in object {
        if let Some(serialized) = json_value_to_ini_token(&value) {
            lines.push(format!("{} = {}", key, serialized));
        }
    }

    Ok(lines.join("\n"))
}

fn json_value_to_ini_token(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::Bool(flag) => Some(if *flag { "Yes" } else { "No" }.to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) => Some(text.clone()),
        Value::Array(values) => {
            let tokens: Vec<String> = values.iter().filter_map(json_value_to_ini_token).collect();
            if tokens.is_empty() {
                None
            } else {
                Some(tokens.join(","))
            }
        }
        Value::Object(_) => None,
    }
}

/// Container manager for tracking all containers in the game
pub struct ContainerManager {
    containers: std::collections::HashMap<ObjectId, Box<dyn ContainerInterface>>,
}

impl ContainerManager {
    /// Create a new container manager
    pub fn new() -> Self {
        Self {
            containers: std::collections::HashMap::new(),
        }
    }

    /// Register a container
    pub fn register_container(
        &mut self,
        object_id: ObjectId,
        container: Box<dyn ContainerInterface>,
    ) {
        self.containers.insert(object_id, container);
    }

    /// Unregister a container
    pub fn unregister_container(&mut self, object_id: &ObjectId) {
        self.containers.remove(object_id);
    }

    /// Get container by object ID
    pub fn get_container(&self, object_id: &ObjectId) -> Option<&dyn ContainerInterface> {
        self.containers.get(object_id).map(|c| c.as_ref())
    }

    /// Get mutable container by object ID
    pub fn get_container_mut(
        &mut self,
        object_id: &ObjectId,
    ) -> Option<&mut dyn ContainerInterface> {
        match self.containers.get_mut(object_id) {
            Some(boxed) => Some(boxed.as_mut()),
            None => None,
        }
    }

    /// Update all containers (called once per frame)
    pub fn update_all(&mut self) -> GameResult<()> {
        // Implementation would update all registered containers
        Ok(())
    }

    /// Get statistics about all containers
    pub fn get_statistics(&self) -> ContainerStatistics {
        let mut stats = ContainerStatistics::default();

        for container in self.containers.values() {
            let (current, _max) = container.get_usage();
            stats.total_containers += 1;
            stats.total_contained_objects += current;

            if container.is_full() {
                stats.full_containers += 1;
            }
            if container.is_empty() {
                stats.empty_containers += 1;
            }
        }

        stats
    }
}

/// Statistics about container usage
#[derive(Debug, Default)]
pub struct ContainerStatistics {
    pub total_containers: u32,
    pub total_contained_objects: u32,
    pub full_containers: u32,
    pub empty_containers: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_manager_creation() {
        let manager = ContainerManager::new();
        assert_eq!(manager.containers.len(), 0);
    }

    #[test]
    fn test_container_statistics() {
        let stats = ContainerStatistics::default();
        assert_eq!(stats.total_containers, 0);
        assert_eq!(stats.total_contained_objects, 0);
    }

    #[test]
    fn test_contain_max_unknown() {
        assert_eq!(CONTAIN_MAX_UNKNOWN, -1);
    }

    #[test]
    fn parse_duration_frames_real_accepts_duration_suffixes() {
        assert_eq!(
            parse_duration_frames_real("1500ms").expect("duration"),
            45.0
        );
        assert_eq!(parse_duration_frames_real("1.5s").expect("duration"), 45.0);
    }
}
