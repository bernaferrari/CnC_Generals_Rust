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
