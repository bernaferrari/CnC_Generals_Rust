//! Receiving weapon definitions shared by worlds using the same startup content.

use std::collections::{HashMap, hash_map::Entry};
use std::fmt;
use std::sync::Arc;

use super::{WeaponDefinition, WeaponDefinitionLoadType, apply_weapon_ini};

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

    /// Load ordered base or override sources and freeze the validated definitions.
    pub fn from_ini_sources<'a>(
        sources: impl IntoIterator<Item = (&'a str, WeaponDefinitionLoadType)>,
    ) -> Result<Self, WeaponCatalogError> {
        let mut definitions = HashMap::<String, Arc<WeaponDefinition>>::new();
        for (text, mode) in sources {
            apply_weapon_ini(&mut definitions, text, mode, |_| {});
        }
        Self::from_named_definitions(definitions)
    }

    /// Build a receiving map catalog. Unchanged weapons retain their shared
    /// definitions. A parse or validation rejection leaves the base available.
    pub fn with_overrides(
        &self,
        sources: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, WeaponCatalogError> {
        let mut definitions = (*self.definitions).clone();
        for text in sources {
            apply_weapon_ini(
                &mut definitions,
                text.as_ref(),
                WeaponDefinitionLoadType::CreateOverrides,
                |_| {},
            );
        }
        Self::from_named_definitions(definitions)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_overrides_share_unchanged_definitions_and_isolate_changed_ones() {
        let base = WeaponCatalog::from_ini_sources([(
            "Weapon TankGun\nPrimaryDamage = 22\nAttackRange = 120\nEnd\n\
             Weapon InfantryGun\nPrimaryDamage = 3\nEnd",
            WeaponDefinitionLoadType::Normal,
        )])
        .unwrap();
        let first_map = base
            .with_overrides(["Weapon tankgun\nPrimaryDamage = 18\nEnd"])
            .unwrap();
        let second_map = base
            .with_overrides(["Weapon TankGun\nPrimaryDamage = 35\nEnd"])
            .unwrap();
        let first = first_map.get("tankgun").unwrap();
        let second = second_map.get("tankgun").unwrap();
        let original = base.get("tankgun").unwrap();
        assert_eq!(
            (
                original.primary_damage,
                first.primary_damage,
                second.primary_damage
            ),
            (22.0, 18.0, 35.0)
        );
        assert_eq!(first.attack_range, original.attack_range);
        assert!(!Arc::ptr_eq(first, original));
        assert!(Arc::ptr_eq(
            base.get("infantrygun").unwrap(),
            first_map.get("infantrygun").unwrap(),
        ));
        assert!(matches!(
            base.with_overrides(["Weapon TankGun\nPrimaryDamage = NaN\nEnd"]),
            Err(WeaponCatalogError::NonFiniteValue)
        ));
        assert_eq!(base.get("tankgun").unwrap().primary_damage, 22.0);
    }
}
