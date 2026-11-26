//! Receiving weapon definitions shared by worlds using the same startup content.

use std::collections::{HashMap, hash_map::Entry};
use std::fmt;
use std::sync::Arc;

use super::WeaponDefinition;

/// Frozen weapon definitions for one base or map content selection.
#[derive(Clone, Debug, Default)]
pub struct WeaponCatalog {
    definitions: Arc<HashMap<String, Arc<WeaponDefinition>>>,
}

impl WeaponCatalog {
    pub fn from_named_definitions<T: Into<Arc<WeaponDefinition>>>(
        definitions: impl IntoIterator<Item = (String, T)>,
    ) -> Result<Self, WeaponCatalogError> {
        let mut catalog = HashMap::new();
        for (name, definition) in definitions {
            let definition = definition.into();
            let name = name.trim();
            if name.is_empty() || !name.eq_ignore_ascii_case(definition.name.trim()) {
                return Err(WeaponCatalogError::InvalidIdentity);
            }
            if !definition.has_finite_simulation_values() {
                return Err(WeaponCatalogError::NonFiniteValue);
            }
            match catalog.entry(name.to_ascii_lowercase()) {
                Entry::Occupied(_) => return Err(WeaponCatalogError::DuplicateIdentity),
                Entry::Vacant(entry) => entry.insert(definition),
            };
        }
        Ok(Self {
            definitions: Arc::new(catalog),
        })
    }

    pub fn get(&self, normalized_name: &str) -> Option<&Arc<WeaponDefinition>> {
        self.definitions.get(normalized_name)
    }

    pub fn contains_key(&self, normalized_name: &str) -> bool {
        self.definitions.contains_key(normalized_name)
    }

    /// Build an unpublished draft before applying ordered Map.ini and Solo.ini rows.
    pub fn override_draft(&self) -> HashMap<String, WeaponDefinition> {
        self.definitions
            .iter()
            .map(|(name, definition)| (name.clone(), (**definition).clone()))
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponCatalogError {
    InvalidIdentity,
    DuplicateIdentity,
    NonFiniteValue,
}

impl fmt::Display for WeaponCatalogError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_str(match self {
            Self::InvalidIdentity => "weapon snapshot has an invalid canonical identity",
            Self::DuplicateIdentity => "weapon snapshot has duplicate effective identities",
            Self::NonFiniteValue => "weapon snapshot has non-finite simulation values",
        })
    }
}

impl std::error::Error for WeaponCatalogError {}
