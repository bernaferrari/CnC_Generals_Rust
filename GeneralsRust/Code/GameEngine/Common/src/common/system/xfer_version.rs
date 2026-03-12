// FILE: xfer_version.rs ///////////////////////////////////////////////////////
// Version compatibility system for save/load
///////////////////////////////////////////////////////////////////////////////

use std::collections::HashMap;
use std::io;

/// Save file version
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SaveVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
    pub build: u16,
}

impl SaveVersion {
    pub const fn new(major: u16, minor: u16, patch: u16, build: u16) -> Self {
        Self {
            major,
            minor,
            patch,
            build,
        }
    }

    pub fn from_u64(value: u64) -> Self {
        Self {
            major: ((value >> 48) & 0xFFFF) as u16,
            minor: ((value >> 32) & 0xFFFF) as u16,
            patch: ((value >> 16) & 0xFFFF) as u16,
            build: (value & 0xFFFF) as u16,
        }
    }

    pub fn to_u64(self) -> u64 {
        ((self.major as u64) << 48)
            | ((self.minor as u64) << 32)
            | ((self.patch as u64) << 16)
            | (self.build as u64)
    }

    pub fn is_compatible_with(&self, other: &SaveVersion) -> bool {
        // Same major version required for compatibility
        // Minor version can differ if current >= saved
        self.major == other.major && self.minor >= other.minor
    }

    pub fn supports_backward_compat(&self, other: &SaveVersion) -> bool {
        // Can load older saves from same major version
        self.major == other.major && self >= other
    }

    pub fn supports_forward_compat(&self, other: &SaveVersion) -> bool {
        // Can load newer saves only if minor version difference <= 1
        if self.major != other.major {
            return false;
        }
        if other.minor > self.minor {
            return other.minor - self.minor <= 1;
        }
        true
    }
}

impl std::fmt::Display for SaveVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}",
            self.major, self.minor, self.patch, self.build
        )
    }
}

/// Current save format version
pub const CURRENT_SAVE_VERSION: SaveVersion = SaveVersion::new(1, 0, 0, 1);

/// Version compatibility checker
pub struct VersionCompatibility {
    current_version: SaveVersion,
    backward_compat_versions: Vec<SaveVersion>,
    forward_compat_versions: Vec<SaveVersion>,
    converters: HashMap<(u64, u64), Box<dyn VersionConverter>>,
}

impl VersionCompatibility {
    pub fn new(current_version: SaveVersion) -> Self {
        Self {
            current_version,
            backward_compat_versions: Vec::new(),
            forward_compat_versions: Vec::new(),
            converters: HashMap::new(),
        }
    }

    /// Check if version is compatible
    pub fn is_compatible(&self, version: &SaveVersion) -> bool {
        if version == &self.current_version {
            return true;
        }

        // Check backward compatibility
        if self.current_version.supports_backward_compat(version) {
            return true;
        }

        // Check forward compatibility
        if self.current_version.supports_forward_compat(version) {
            return true;
        }

        // Check explicit compatibility lists
        self.backward_compat_versions.contains(version)
            || self.forward_compat_versions.contains(version)
    }

    /// Add backward compatible version
    pub fn add_backward_compatible(&mut self, version: SaveVersion) {
        if !self.backward_compat_versions.contains(&version) {
            self.backward_compat_versions.push(version);
            self.backward_compat_versions.sort();
        }
    }

    /// Add forward compatible version
    pub fn add_forward_compatible(&mut self, version: SaveVersion) {
        if !self.forward_compat_versions.contains(&version) {
            self.forward_compat_versions.push(version);
            self.forward_compat_versions.sort();
        }
    }

    /// Register version converter
    pub fn register_converter<C: VersionConverter + 'static>(
        &mut self,
        from_version: SaveVersion,
        to_version: SaveVersion,
        converter: C,
    ) {
        let key = (from_version.to_u64(), to_version.to_u64());
        self.converters.insert(key, Box::new(converter));
    }

    /// Find converter for version pair
    pub fn find_converter(
        &self,
        from_version: SaveVersion,
        to_version: SaveVersion,
    ) -> Option<&dyn VersionConverter> {
        let key = (from_version.to_u64(), to_version.to_u64());
        self.converters.get(&key).map(|b| b.as_ref())
    }

    /// Get current version
    pub fn current_version(&self) -> SaveVersion {
        self.current_version
    }

    /// Check if needs conversion
    pub fn needs_conversion(&self, saved_version: SaveVersion) -> bool {
        saved_version != self.current_version
            && self
                .find_converter(saved_version, self.current_version)
                .is_some()
    }
}

impl Default for VersionCompatibility {
    fn default() -> Self {
        Self::new(CURRENT_SAVE_VERSION)
    }
}

/// Trait for version converters
pub trait VersionConverter {
    /// Convert data from one version to another
    fn convert(&self, data: &[u8]) -> io::Result<Vec<u8>>;

    /// Get source version
    fn from_version(&self) -> SaveVersion;

    /// Get target version
    fn to_version(&self) -> SaveVersion;

    /// Get conversion description
    fn description(&self) -> &str {
        "Version conversion"
    }
}

/// Version migration record
#[derive(Debug, Clone)]
pub struct VersionMigration {
    pub from_version: SaveVersion,
    pub to_version: SaveVersion,
    pub description: String,
    pub migration_date: std::time::SystemTime,
}

/// Version migration manager
pub struct MigrationManager {
    migrations: Vec<VersionMigration>,
    active_migrations: HashMap<u64, VersionMigration>,
}

impl MigrationManager {
    pub fn new() -> Self {
        Self {
            migrations: Vec::new(),
            active_migrations: HashMap::new(),
        }
    }

    /// Record migration
    pub fn record_migration(&mut self, migration: VersionMigration) {
        let key = migration.from_version.to_u64();
        self.active_migrations.insert(key, migration.clone());
        self.migrations.push(migration);
    }

    /// Get migration for version
    pub fn get_migration(&self, from_version: SaveVersion) -> Option<&VersionMigration> {
        self.active_migrations.get(&from_version.to_u64())
    }

    /// Get all migrations
    pub fn get_all_migrations(&self) -> &[VersionMigration] {
        &self.migrations
    }

    /// Clear migration history
    pub fn clear(&mut self) {
        self.migrations.clear();
        self.active_migrations.clear();
    }
}

impl Default for MigrationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Field version annotations for selective loading
#[derive(Debug, Clone)]
pub struct FieldVersion {
    pub field_name: String,
    pub added_in: SaveVersion,
    pub removed_in: Option<SaveVersion>,
    pub default_value: Option<Vec<u8>>,
}

impl FieldVersion {
    pub fn new(field_name: String, added_in: SaveVersion) -> Self {
        Self {
            field_name,
            added_in,
            removed_in: None,
            default_value: None,
        }
    }

    pub fn with_removal(mut self, removed_in: SaveVersion) -> Self {
        self.removed_in = Some(removed_in);
        self
    }

    pub fn with_default(mut self, default_value: Vec<u8>) -> Self {
        self.default_value = Some(default_value);
        self
    }

    /// Check if field exists in given version
    pub fn exists_in_version(&self, version: &SaveVersion) -> bool {
        if version < &self.added_in {
            return false;
        }
        if let Some(removed_in) = &self.removed_in {
            if version >= removed_in {
                return false;
            }
        }
        true
    }

    /// Check if field needs default value for given version
    pub fn needs_default(&self, version: &SaveVersion) -> bool {
        !self.exists_in_version(version) && self.default_value.is_some()
    }
}

/// Version-aware field registry
pub struct FieldRegistry {
    fields: HashMap<String, FieldVersion>,
}

impl FieldRegistry {
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
        }
    }

    /// Register field version info
    pub fn register_field(&mut self, field: FieldVersion) {
        self.fields.insert(field.field_name.clone(), field);
    }

    /// Get field version info
    pub fn get_field(&self, field_name: &str) -> Option<&FieldVersion> {
        self.fields.get(field_name)
    }

    /// Check if field should be loaded for version
    pub fn should_load_field(&self, field_name: &str, version: &SaveVersion) -> bool {
        self.fields
            .get(field_name)
            .map(|f| f.exists_in_version(version))
            .unwrap_or(true) // Unknown fields are loaded by default
    }

    /// Get default value for field if needed
    pub fn get_default_value(&self, field_name: &str, version: &SaveVersion) -> Option<&[u8]> {
        self.fields.get(field_name).and_then(|f| {
            if f.needs_default(version) {
                f.default_value.as_deref()
            } else {
                None
            }
        })
    }
}

impl Default for FieldRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_version() {
        let v1 = SaveVersion::new(1, 2, 3, 4);
        assert_eq!(v1.major, 1);
        assert_eq!(v1.minor, 2);
        assert_eq!(v1.patch, 3);
        assert_eq!(v1.build, 4);

        let encoded = v1.to_u64();
        let v2 = SaveVersion::from_u64(encoded);
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_version_compatibility() {
        let v1_0 = SaveVersion::new(1, 0, 0, 0);
        let v1_1 = SaveVersion::new(1, 1, 0, 0);
        let v1_2 = SaveVersion::new(1, 2, 0, 0);
        let v2_0 = SaveVersion::new(2, 0, 0, 0);

        assert!(v1_1.is_compatible_with(&v1_0)); // Can load older minor
        assert!(!v1_0.is_compatible_with(&v1_1)); // Cannot load newer minor
        assert!(!v1_0.is_compatible_with(&v2_0)); // Cannot load different major
    }

    #[test]
    fn test_backward_forward_compat() {
        let v1_0 = SaveVersion::new(1, 0, 0, 0);
        let v1_1 = SaveVersion::new(1, 1, 0, 0);
        let v1_2 = SaveVersion::new(1, 2, 0, 0);

        assert!(v1_2.supports_backward_compat(&v1_0));
        assert!(v1_2.supports_backward_compat(&v1_1));

        assert!(v1_1.supports_forward_compat(&v1_2));
        assert!(!v1_0.supports_forward_compat(&v1_2));
    }

    #[test]
    fn test_field_version() {
        let v1_0 = SaveVersion::new(1, 0, 0, 0);
        let v1_5 = SaveVersion::new(1, 5, 0, 0);
        let v2_0 = SaveVersion::new(2, 0, 0, 0);

        let field = FieldVersion::new("test_field".to_string(), v1_5).with_removal(v2_0);

        assert!(!field.exists_in_version(&v1_0)); // Before added
        assert!(field.exists_in_version(&v1_5)); // When added
        assert!(!field.exists_in_version(&v2_0)); // After removed
    }

    #[test]
    fn test_field_registry() {
        let mut registry = FieldRegistry::new();

        let field = FieldVersion::new("new_feature".to_string(), SaveVersion::new(1, 5, 0, 0));

        registry.register_field(field);

        let v1_0 = SaveVersion::new(1, 0, 0, 0);
        let v1_5 = SaveVersion::new(1, 5, 0, 0);

        assert!(!registry.should_load_field("new_feature", &v1_0));
        assert!(registry.should_load_field("new_feature", &v1_5));
    }
}
