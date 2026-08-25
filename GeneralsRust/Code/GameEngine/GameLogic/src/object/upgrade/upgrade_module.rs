use game_engine::common::system::Xfer;

pub use crate::upgrade::modules::upgrade_mux::{
    UpgradeModuleInterface, UpgradeMux, UpgradeMuxData,
};

use crate::common::{ObjectID, UpgradeMaskType};
use crate::object::Object;
use crate::upgrade::UpgradeMask;

/// Gate a live upgrade module the same way C++ `UpgradeMux::wouldUpgrade` does.
pub fn mux_can_upgrade(
    data: &UpgradeMuxData,
    applied: bool,
    upgrade_mask: UpgradeMaskType,
) -> bool {
    if applied || upgrade_mask.is_empty() {
        return false;
    }
    let key_mask = UpgradeMask::from_bits_retain(upgrade_mask.bits());
    let mut mux = UpgradeMux::new(data.clone());
    mux.set_upgrade_executed(applied);
    mux.would_upgrade(key_mask)
}

/// C++ `UpgradeMux::giveSelfUpgrade` FX + RemovesUpgrades, before implementation.
pub fn mux_give_self_upgrade(data: &UpgradeMuxData, object: &mut Object) {
    data.perform_upgrade_fx(object);
    data.process_upgrade_removal(object);
}

/// C++ `UpgradeMux::resetUpgrade` (`UpgradeModule.cpp:191-201`).
/// Clears `m_upgradeExecuted` / leftover `applied` when `keyMask` intersects
/// the module activation mask. Does **not** undo `upgradeImplementation`.
pub fn mux_reset_upgrade(
    data: &UpgradeMuxData,
    applied: &mut bool,
    upgrade_mask: UpgradeMaskType,
) -> bool {
    if !*applied || upgrade_mask.is_empty() {
        return false;
    }
    let key_mask = UpgradeMask::from_bits_retain(upgrade_mask.bits());
    let mut mux = UpgradeMux::new(data.clone());
    mux.set_upgrade_executed(*applied);
    if mux.reset_upgrade(key_mask) {
        *applied = false;
        true
    } else {
        false
    }
}

/// Look up the owning object and run `giveSelfUpgrade` bookkeeping.
pub fn mux_give_self_upgrade_for_object(data: &UpgradeMuxData, object_id: ObjectID) {
    let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(object_id, |object| {
        mux_give_self_upgrade(data, object);
    });
}

/// INI parsers that write mux keys into `$data_ty.upgrade_mux_data`.
#[macro_export]
macro_rules! impl_upgrade_mux_field_parsers {
    ($data_ty:ty) => {
        fn parse_mux_triggered_by(
            _ini: &mut game_engine::common::ini::INI,
            data: &mut $data_ty,
            tokens: &[&str],
        ) -> Result<(), game_engine::common::ini::INIError> {
            data.upgrade_mux_data.parse_triggered_by_tokens(tokens)
        }
        fn parse_mux_conflicts_with(
            _ini: &mut game_engine::common::ini::INI,
            data: &mut $data_ty,
            tokens: &[&str],
        ) -> Result<(), game_engine::common::ini::INIError> {
            data.upgrade_mux_data.parse_conflicts_with_tokens(tokens)
        }
        fn parse_mux_removes_upgrades(
            _ini: &mut game_engine::common::ini::INI,
            data: &mut $data_ty,
            tokens: &[&str],
        ) -> Result<(), game_engine::common::ini::INIError> {
            data.upgrade_mux_data.parse_removes_upgrades_tokens(tokens)
        }
        fn parse_mux_requires_all_triggers(
            _ini: &mut game_engine::common::ini::INI,
            data: &mut $data_ty,
            tokens: &[&str],
        ) -> Result<(), game_engine::common::ini::INIError> {
            data.upgrade_mux_data
                .parse_requires_all_triggers_tokens(tokens)
        }
        fn parse_mux_fx_list_upgrade(
            _ini: &mut game_engine::common::ini::INI,
            data: &mut $data_ty,
            tokens: &[&str],
        ) -> Result<(), game_engine::common::ini::INIError> {
            data.upgrade_mux_data.parse_fx_list_upgrade_tokens(tokens)
        }
    };
}

/// Complete field table: mux keys plus any module-specific `FieldParse`s.
#[macro_export]
macro_rules! upgrade_mux_field_table {
    ($($extra:expr),* $(,)?) => {
        &[
            game_engine::common::ini::FieldParse {
                token: "TriggeredBy",
                parse: parse_mux_triggered_by,
            },
            game_engine::common::ini::FieldParse {
                token: "ConflictsWith",
                parse: parse_mux_conflicts_with,
            },
            game_engine::common::ini::FieldParse {
                token: "RemovesUpgrades",
                parse: parse_mux_removes_upgrades,
            },
            game_engine::common::ini::FieldParse {
                token: "RequiresAllTriggers",
                parse: parse_mux_requires_all_triggers,
            },
            game_engine::common::ini::FieldParse {
                token: "FXListUpgrade",
                parse: parse_mux_fx_list_upgrade,
            },
            $($extra,)*
        ]
    };
}

pub(crate) fn xfer_upgrade_module_state(
    xfer: &mut dyn Xfer,
    upgrade_executed: &mut bool,
) -> Result<(), String> {
    // C++ UpgradeModule::xfer chains through BehaviorModule, ObjectModule, Module,
    // then UpgradeMux::upgradeMuxXfer. Each layer writes its own version byte.
    let mut upgrade_module_version: u8 = 1;
    xfer.xfer_version(&mut upgrade_module_version, 1)
        .map_err(|e| e.to_string())?;

    let mut behavior_module_version: u8 = 1;
    xfer.xfer_version(&mut behavior_module_version, 1)
        .map_err(|e| e.to_string())?;

    let mut object_module_version: u8 = 1;
    xfer.xfer_version(&mut object_module_version, 1)
        .map_err(|e| e.to_string())?;

    let mut module_version: u8 = 1;
    xfer.xfer_version(&mut module_version, 1)
        .map_err(|e| e.to_string())?;

    let mut upgrade_mux_version: u8 = 1;
    xfer.xfer_version(&mut upgrade_mux_version, 1)
        .map_err(|e| e.to_string())?;

    xfer.xfer_bool(upgrade_executed)
        .map_err(|e| e.to_string())?;
    Ok(())
}

// C++ UpgradeModule::crc calls BehaviorModule::crc then UpgradeMux::upgradeMuxCRC,
// which delegates to upgradeMuxXfer — identical code path to xfer.
pub(crate) fn crc_upgrade_module_state(
    xfer: &mut dyn Xfer,
    upgrade_executed: bool,
) -> Result<(), String> {
    let mut executed = upgrade_executed;
    xfer_upgrade_module_state(xfer, &mut executed)
}

pub(crate) fn xfer_upgrade_module_with_version(
    xfer: &mut dyn Xfer,
    upgrade_executed: &mut bool,
    module_name: &str,
) -> Result<(), String> {
    let mut version: u8 = 1;
    xfer.xfer_version(&mut version, 1)
        .map_err(|err| format!("{module_name} xfer version: {err:?}"))?;
    xfer_upgrade_module_state(xfer, upgrade_executed)
        .map_err(|err| format!("{module_name} xfer upgrade module state: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;

    #[test]
    fn upgrade_module_xfer_preserves_executed_state() {
        let mut saved = true;
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut xfer = XferSave::new(&mut bytes, 1);
            xfer_upgrade_module_with_version(&mut xfer, &mut saved, "TestUpgrade").unwrap();
        }

        bytes.set_position(0);
        let mut loaded = false;
        {
            let mut xfer = XferLoad::new(&mut bytes, 1);
            xfer_upgrade_module_with_version(&mut xfer, &mut loaded, "TestUpgrade").unwrap();
        }

        assert!(loaded);
    }

    #[test]
    fn mux_reset_upgrade_clears_applied_when_activation_intersects() {
        let mut data = UpgradeMuxData::default();
        data.activation_upgrade_names
            .push(game_engine::common::ascii_string::AsciiString::from(
                "ResetUpgradeA",
            ));
        let mask = UpgradeMaskType::from_bits_retain(
            crate::upgrade::upgrade_mask_for_name("ResetUpgradeA").to_bits(),
        );
        let mut applied = true;
        assert!(mux_reset_upgrade(&data, &mut applied, mask));
        assert!(!applied);
        assert!(!mux_reset_upgrade(&data, &mut applied, mask));
    }
}
