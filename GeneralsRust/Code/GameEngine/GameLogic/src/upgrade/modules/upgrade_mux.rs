//! Upgrade Mux System
//!
//! Base functionality for modules that respond to upgrade triggers.
//! Matches C++ UpgradeMux and UpgradeMuxData from UpgradeModule.h/.cpp
//!
//! Original C++ Authors: Johnson, Day, Smallwood, September 2002

use super::super::{UpgradeMask, upgrade_mask_for_name};

/// C++ Module.cpp:277-282 / 291-296 — unknown nonempty non-None names throw INI_INVALID_DATA.
fn resolve_named_upgrade_mask(name: &str) -> UpgradeMask {
    if name.is_empty() || name.eq_ignore_ascii_case("None") {
        return UpgradeMask::none();
    }
    let center = super::super::center::get_upgrade_center();
    let center = center
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match center.mask_for_name(name) {
        Some(mask) => mask,
        None => panic!(
            "An upgrade module references {name}, which is not an Upgrade (C++ INI_INVALID_DATA)"
        ),
    }
}

use crate::common::*;
#[cfg(test)]
use game_engine::common::ini::INI;
pub use game_engine::common::system::upgrade::UpgradeMuxData;
#[cfg(test)]
use game_engine::common::system::upgrade::UpgradeMuxDefinition;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::ModuleDataView;
use generals_sim::upgrade::UpgradeConditions;

/// Interface for upgrade modules
/// Matches C++ UpgradeModuleInterface from UpgradeModule.h
pub trait UpgradeModuleInterface {
    /// Check if upgrade has already been applied
    fn is_already_upgraded(&self) -> bool;

    /// Attempt to apply upgrade with given mask
    /// Returns true if upgrade was triggered
    fn attempt_upgrade(&mut self, key_mask: UpgradeMask, object: &mut Object) -> bool;

    /// Check if this upgrade would trigger with given mask (without applying)
    fn would_upgrade(&self, key_mask: UpgradeMask) -> bool;

    /// Reset upgrade state (for conflicting upgrades)
    fn reset_upgrade(&mut self, key_mask: UpgradeMask) -> bool;

    /// Test if upgrade conditions are met
    fn test_upgrade_conditions(&self, key_mask: UpgradeMask) -> bool;

    /// Force refresh of already-applied upgrade
    fn force_refresh_upgrade(&mut self, object: &mut Object);
}

/// Resolve upgrade masks and dispatch effects through the legacy runtime.
pub trait UpgradeMuxRuntime {
    fn get_upgrade_activation_masks(&mut self) -> (UpgradeMask, UpgradeMask);
    fn perform_upgrade_fx(&self, object: &mut Object);
    fn process_upgrade_removal(&self, object: &mut Object);
}

impl UpgradeMuxRuntime for UpgradeMuxData {
    fn get_upgrade_activation_masks(&mut self) -> (UpgradeMask, UpgradeMask) {
        self.cache_activation_masks_with(resolve_named_upgrade_mask)
    }

    /// Perform upgrade FX
    /// Matches C++ UpgradeMuxData::performUpgradeFX
    fn perform_upgrade_fx(&self, object: &mut Object) {
        if let Some(fx_name) = &self.definition.fx_list_name {
            if let Some(fx_mgr) = crate::helpers::get_fx_list_manager() {
                fx_mgr.do_fx_for_object(
                    NameKeyGenerator::name_to_key(fx_name.as_str()) as FXListId,
                    object,
                );
            } else {
                log::debug!(
                    "Upgrade FX list requested for object {}, but FX manager is not registered",
                    object.get_id()
                );
            }
        }
    }

    /// Process upgrade removals
    /// Matches C++ UpgradeMuxData::muxDataProcessUpgradeRemoval
    fn process_upgrade_removal(&self, object: &mut Object) {
        for name in &self.definition.removal_upgrade_names {
            let mask = upgrade_mask_for_name(name.as_str());

            // Convert UpgradeMask to UpgradeMaskType for object methods
            let mask_bits = crate::common::UpgradeMaskType::from_bits_retain(mask.to_bits());

            // Remove conflicting upgrades from object
            // Matches C++ UpgradeMuxData::muxDataProcessUpgradeRemoval
            // which calls obj->loseUpgrade(mask) for each removal
            object.remove_upgrade_mask(mask_bits);

            log::debug!(
                "Removed conflicting upgrade {} (mask {:?}) from object {}",
                name,
                mask,
                object.get_id()
            );
        }
    }
}

enum UpgradeMuxDataRead<'a> {
    Borrowed(&'a UpgradeMuxData),
    Shared(parking_lot::MappedRwLockReadGuard<'a, UpgradeMuxData>),
}

impl std::ops::Deref for UpgradeMuxDataRead<'_> {
    type Target = UpgradeMuxData;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(data) => data,
            Self::Shared(data) => data,
        }
    }
}

/// Upgrade mux state and behavior
/// Matches C++ UpgradeMux from UpgradeModule.h/.cpp
#[derive(Debug)]
pub struct UpgradeMux {
    /// Configuration data
    pub data: UpgradeMuxData,
    /// Live configuration for muxes embedded in admitted module data.
    shared_data: Option<ModuleDataView<UpgradeMuxData>>,
    /// Whether upgrade has been executed
    upgrade_executed: bool,
}

impl UpgradeMux {
    /// Create new upgrade mux
    /// Matches C++ UpgradeMux::UpgradeMux
    pub fn new(data: UpgradeMuxData) -> Self {
        Self {
            data,
            shared_data: None,
            upgrade_executed: false,
        }
    }

    /// Create a mux backed by a live projection of its owning module data.
    pub fn new_shared(data: ModuleDataView<UpgradeMuxData>) -> Self {
        Self {
            data: UpgradeMuxData::default(),
            shared_data: Some(data),
            upgrade_executed: false,
        }
    }

    /// Capture the current configuration without holding its lock during effects.
    pub fn data_snapshot(&self) -> UpgradeMuxData {
        match &self.shared_data {
            Some(data) => data.read().clone(),
            None => self.data.clone(),
        }
    }

    fn current_data(&self) -> UpgradeMuxDataRead<'_> {
        match &self.shared_data {
            Some(data) => UpgradeMuxDataRead::Shared(data.read()),
            None => UpgradeMuxDataRead::Borrowed(&self.data),
        }
    }

    pub(crate) fn is_triggered_by(&self, upgrade: &str) -> bool {
        self.current_data().definition.is_triggered_by(upgrade)
    }

    pub(crate) fn activation_masks(&self) -> (UpgradeMask, UpgradeMask) {
        self.current_data()
            .activation_masks_with(resolve_named_upgrade_mask)
    }

    /// Check if already upgraded
    /// Matches C++ UpgradeMux::isAlreadyUpgraded
    pub fn is_already_upgraded(&self) -> bool {
        self.upgrade_executed
    }

    /// Set upgraded state
    pub fn set_upgrade_executed(&mut self, executed: bool) {
        self.upgrade_executed = executed;
    }

    /// Attempt upgrade
    /// Matches C++ UpgradeMux::attemptUpgrade
    pub fn attempt_upgrade(&mut self, key_mask: UpgradeMask, object: &mut Object) -> bool {
        if self.would_upgrade(key_mask) {
            self.give_self_upgrade(object);
            true
        } else {
            false
        }
    }

    /// Check if would upgrade
    /// Matches C++ UpgradeMux::wouldUpgrade
    pub fn would_upgrade(&self, key_mask: UpgradeMask) -> bool {
        let conditions = self.current_conditions();
        !self.upgrade_executed && conditions.can_trigger(key_mask.bits())
    }

    /// Give self upgrade
    /// Matches C++ UpgradeMux::giveSelfUpgrade
    fn give_self_upgrade(&mut self, object: &mut Object) {
        let data = self.data_snapshot();
        data.perform_upgrade_fx(object);
        data.process_upgrade_removal(object);
        // Actual upgrade implementation is handled by subclass
        self.set_upgrade_executed(true);
    }

    /// Test upgrade conditions
    /// Matches C++ UpgradeMux::testUpgradeConditions
    pub fn test_upgrade_conditions(&self, key_mask: UpgradeMask) -> bool {
        self.current_conditions().is_satisfied(key_mask.bits())
    }

    fn current_conditions(&self) -> UpgradeConditions {
        let data = self.current_data();
        let (activation, conflicts) = data.activation_masks_with(resolve_named_upgrade_mask);
        UpgradeConditions {
            activation: activation.bits(),
            conflicts: conflicts.bits(),
            requires_all_triggers: data.definition.requires_all_triggers,
        }
    }

    /// Reset upgrade
    /// Matches C++ UpgradeMux::resetUpgrade
    pub fn reset_upgrade(&mut self, key_mask: UpgradeMask) -> bool {
        let (activation, _) = self
            .current_data()
            .activation_masks_with(resolve_named_upgrade_mask);

        if key_mask.test_for_any(activation) && self.upgrade_executed {
            self.upgrade_executed = false;
            true
        } else {
            false
        }
    }

    /// Force refresh upgrade
    /// Matches C++ UpgradeMux::forceRefreshUpgrade
    pub fn force_refresh_upgrade(&mut self, object: &mut Object) {
        if self.upgrade_executed {
            // Only refresh if already upgraded
            // Actual implementation is in subclass
            log::debug!("Force refreshing upgrade for object {}", object.get_id());
        }
    }
}

impl Snapshotable for UpgradeMux {
    /// CRC for save game validation
    /// Matches C++ UpgradeMux::upgradeMuxCRC
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // CRC calculation for save game validation
        // Matches C++ UpgradeMux::upgradeMuxCRC
        // CRC only reads state for validation, doesn't modify it
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;

        // CRC the upgrade_executed flag
        // Note: In CRC mode, xfer is read-only but needs mutable reference
        // This is a limitation of the Xfer trait design
        let mut upgrade_executed = self.upgrade_executed;
        xfer.xfer_bool(&mut upgrade_executed)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Serialize/deserialize
    /// Matches C++ UpgradeMux::upgradeMuxXfer
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // Version 1
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;

        // Serialize upgrade_executed flag
        xfer.xfer_bool(&mut self.upgrade_executed)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Post-load processing
    /// Matches C++ UpgradeMux::upgradeMuxLoadPostProcess
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

// INI parsing functions
#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    fn authored_catalog_child(name: &str) -> bool {
        let name = name.strip_prefix("gamelogic::").unwrap_or(name);
        matches!(
            crate::test_process::run_bounded(name, "GENERALS_UPGRADE_MUX_CATALOG_CHILD"),
            crate::test_process::TestProcess::Child
        )
    }

    #[cfg(target_arch = "wasm32")]
    fn authored_catalog_child(_: &str) -> bool {
        true
    }

    fn declare_activation_and_conflict() {
        crate::upgrade::center::with_upgrade_center_mut(|center| {
            let a = center.new_upgrade(AsciiString::from("UpgradeA"));
            let b = center.new_upgrade(AsciiString::from("UpgradeB"));
            assert!(a.mask().any() && b.mask().any());
            assert!(!a.mask().intersects(b.mask()));
        });
    }

    fn make_test_mask_with_bit(bit: usize) -> UpgradeMask {
        let mut mask = UpgradeMask::none();
        mask.set_bit(bit);
        mask
    }

    #[test]
    fn test_upgrade_mux_basic() {
        let data = UpgradeMuxData::from_definition(UpgradeMuxDefinition {
            activation_upgrade_names: vec![AsciiString::from("TestUpgrade")],
            ..Default::default()
        });

        let mux = UpgradeMux::new(data);
        assert!(!mux.is_already_upgraded());
    }

    #[test]
    fn test_would_upgrade() {
        crate::upgrade::test_upgrade_mask("TestUpgrade");
        let mut data = UpgradeMuxData::from_definition(UpgradeMuxDefinition {
            activation_upgrade_names: vec![AsciiString::from("TestUpgrade")],
            requires_all_triggers: false,
            ..Default::default()
        });

        // Get masks
        let (activation, _) = data.get_upgrade_activation_masks();

        let mux = UpgradeMux::new(data);
        assert!(mux.would_upgrade(activation));
    }

    #[test]
    fn test_conflicting_upgrades() {
        if !authored_catalog_child(concat!(module_path!(), "::test_conflicting_upgrades")) {
            return;
        }
        let _guard = crate::test_sync::lock();
        // C++ resolves declared UpgradeCenter templates, not invented masks.
        declare_activation_and_conflict();
        let mut data = UpgradeMuxData::from_definition(UpgradeMuxDefinition {
            activation_upgrade_names: vec![AsciiString::from("UpgradeA")],
            conflicting_upgrade_names: vec![AsciiString::from("UpgradeB")],
            requires_all_triggers: false,
            ..Default::default()
        });

        let (activation, conflicting) = data.get_upgrade_activation_masks();

        let mux = UpgradeMux::new(data);

        // With only activation, should work
        assert!(mux.would_upgrade(activation));

        // With activation + conflict, should not work
        let combined = activation | conflicting;
        assert!(!mux.would_upgrade(combined));
    }

    #[test]
    fn test_requires_all_triggers() {
        if !authored_catalog_child(concat!(module_path!(), "::test_requires_all_triggers")) {
            return;
        }
        let _guard = crate::test_sync::lock();
        // C++ resolves declared UpgradeCenter templates, not invented masks.
        declare_activation_and_conflict();
        let mut data = UpgradeMuxData::from_definition(UpgradeMuxDefinition {
            activation_upgrade_names: vec![
                AsciiString::from("UpgradeA"),
                AsciiString::from("UpgradeB"),
            ],
            requires_all_triggers: true,
            ..Default::default()
        });

        let (activation, _) = data.get_upgrade_activation_masks();

        let mux = UpgradeMux::new(data);

        // Must have ALL bits set
        assert!(mux.would_upgrade(activation));

        // With only partial, should fail
        let mask_a = upgrade_mask_for_name("UpgradeA");
        assert!(!mux.would_upgrade(mask_a));
    }

    #[test]
    fn parse_fx_list_upgrade_stores_named_list() {
        let mut data = UpgradeMuxData::default();
        data.definition
            .parse_fx_list_upgrade_tokens(&["FX_UpgradeFlash"])
            .expect("parse FXListUpgrade");
        assert_eq!(
            data.definition
                .fx_list_name
                .as_ref()
                .map(AsciiString::as_str),
            Some("FX_UpgradeFlash")
        );
        data.definition
            .parse_fx_list_upgrade_tokens(&["None"])
            .expect("parse None");
        assert!(data.definition.fx_list_name.is_none());
    }

    /// C++ UpgradeMux::testUpgradeConditions: ConflictsWith is not an always-off switch.
    #[test]
    fn conflicts_with_listed_does_not_block_empty_completed_mask() {
        use game_engine::common::common_services::{CommonServices, with_common_services};
        use std::sync::Arc;
        let services = Arc::new(CommonServices::new());
        with_common_services(&services, || {
            let stores = Arc::new(crate::system::engine_stores::EngineStores::new_for_world());
            crate::system::engine_stores::with_active_stores(&stores, || {
                crate::upgrade::test_upgrade_mask("Upgrade_HE");
                let data = UpgradeMuxData::from_definition(UpgradeMuxDefinition {
                    conflicting_upgrade_names: vec![AsciiString::from("Upgrade_HE")],
                    ..Default::default()
                });
                let mux = UpgradeMux::new(data);
                assert!(
                    mux.test_upgrade_conditions(UpgradeMask::none()),
                    "C++ onDie only skips when object/player completed mask intersects ConflictsWith"
                );
            });
        });
    }

    #[test]
    #[test]
    #[should_panic(expected = "which is not an Upgrade")]
    fn unknown_upgrade_name_fails_on_first_mask_resolve() {
        use game_engine::common::common_services::{CommonServices, with_common_services};
        use std::sync::Arc;
        let services = Arc::new(CommonServices::new());
        with_common_services(&services, || {
            let stores = Arc::new(crate::system::engine_stores::EngineStores::new_for_world());
            crate::system::engine_stores::with_active_stores(&stores, || {
                let mut data = UpgradeMuxData::default();
                data.definition
                    .activation_upgrade_names
                    .push(AsciiString::from("Upgrade_DefinitelyMissingForMux"));
                let _ = data.get_upgrade_activation_masks();
            });
        });
    }

    #[test]
    fn upgrade_mux_repeated_vector_fields_replace_instead_of_append() {
        let mut data = UpgradeMuxData::from_definition(UpgradeMuxDefinition {
            trigger_upgrade_names: vec![AsciiString::from("LegacyTrigger")],
            activation_upgrade_names: vec![AsciiString::from("StaleActivation")],
            conflicting_upgrade_names: vec![AsciiString::from("StaleConflict")],
            removal_upgrade_names: vec![AsciiString::from("StaleRemoval")],
            ..Default::default()
        });
        // The actual canonical field table invokes parseAsciiStringVector's
        // replacement operation for every occurrence, in source order.
        INI::new()
            .with_inline_source(
                "TriggeredBy = FirstA FirstB\nConflictsWith = FirstConflict\n\
                 RemovesUpgrades = FirstRemoval\nTriggeredBy = FinalA FinalB\n\
                 ConflictsWith = FinalConflict\nRemovesUpgrades = FinalRemoval\n\
                 RequiresAllTriggers = Yes\nEnd\nFollowingBlock\n",
                |ini| {
                    data.parse_from_ini(ini)?;
                    ini.read_line()?;
                    assert_eq!(ini.get_line_tokens(), ["FollowingBlock"]);
                    Ok(())
                },
            )
            .unwrap();
        let names = |values: &[AsciiString]| {
            values
                .iter()
                .map(|v| v.as_str().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            (
                names(&data.definition.activation_upgrade_names),
                names(&data.definition.conflicting_upgrade_names),
                names(&data.definition.removal_upgrade_names),
            ),
            (
                vec!["FinalA".to_owned(), "FinalB".to_owned()],
                vec!["FinalConflict".to_owned()],
                vec!["FinalRemoval".to_owned()],
            ),
            "C++ replacing vector fields keep only each last authored list"
        );
        assert_eq!(
            names(&data.definition.trigger_upgrade_names),
            ["LegacyTrigger"]
        );
        assert!(data.definition.requires_all_triggers);
    }

    #[test]
    fn upgrade_mux_empty_vector_fields_clear_only_the_selected_lists() {
        let mut data = UpgradeMuxData::from_definition(UpgradeMuxDefinition {
            trigger_upgrade_names: vec![AsciiString::from("LegacyTrigger")],
            ..Default::default()
        });
        data.definition
            .parse_triggered_by_tokens(&["=", "BeforeA"])
            .unwrap();
        data.definition
            .parse_conflicts_with_tokens(&["=", "BeforeConflict"])
            .unwrap();
        data.definition
            .parse_removes_upgrades_tokens(&["=", "BeforeRemoval"])
            .unwrap();
        INI::new()
            .with_inline_source(
                "TriggeredBy =\nConflictsWith =\nRemovesUpgrades =\nEnd\n",
                |ini| data.parse_from_ini(ini),
            )
            .unwrap();
        assert_eq!(
            (
                data.definition.activation_upgrade_names.len(),
                data.definition.conflicting_upgrade_names.len(),
                data.definition.removal_upgrade_names.len(),
            ),
            (0, 0, 0),
            "C++ empty vector fields clear all three previously populated lists"
        );
        // The direct token adapters retain '=' handling and clear only their
        // target. No legacy trigger list, mask resolution, world, or FX owner
        // is mutated to exercise definition parsing.
        data.definition
            .parse_triggered_by_tokens(&["=", "KeepA"])
            .unwrap();
        data.definition
            .parse_conflicts_with_tokens(&["=", "KeepConflict"])
            .unwrap();
        data.definition
            .parse_removes_upgrades_tokens(&["=", "KeepRemoval"])
            .unwrap();
        data.definition.parse_conflicts_with_tokens(&["="]).unwrap();
        assert_eq!(
            data.definition.activation_upgrade_names[0].as_str(),
            "KeepA"
        );
        assert!(data.definition.conflicting_upgrade_names.is_empty());
        assert_eq!(
            data.definition.removal_upgrade_names[0].as_str(),
            "KeepRemoval"
        );
        data.definition.parse_triggered_by_tokens(&[]).unwrap();
        data.definition
            .parse_removes_upgrades_tokens(&["="])
            .unwrap();
        assert!(data.definition.activation_upgrade_names.is_empty());
        assert!(data.definition.removal_upgrade_names.is_empty());
        assert_eq!(data.definition.trigger_upgrade_names.len(), 1);
        assert_eq!(
            data.definition.trigger_upgrade_names[0].as_str(),
            "LegacyTrigger"
        );
    }
}
