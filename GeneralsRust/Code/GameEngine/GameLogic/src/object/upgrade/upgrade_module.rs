use game_engine::common::system::Xfer;

pub use crate::upgrade::modules::upgrade_mux::{
    UpgradeModuleInterface, UpgradeMux, UpgradeMuxData,
};

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
}
