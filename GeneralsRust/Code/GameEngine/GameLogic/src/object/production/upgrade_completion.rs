//! ProductionUpdate.cpp:874–956 object-upgrade grant phase. The notification,
//! accounting and research sounds remain in the production executor around it.
use crate::common::UpgradeMaskType;
use crate::object::Object;
use crate::upgrade::UpgradeTemplate;

/// Same grant implementation for compatibility and instance-owned production
/// execution. No object registry, player list or frame selection occurs here.
pub(crate) fn grant_object_upgrade(
    owner: &mut Object,
    upgrade: &UpgradeTemplate,
    completed_player_upgrades: UpgradeMaskType,
    radar_grant: &mut dyn FnMut(&mut Object, bool),
) {
    owner.give_upgrade_with_radar(upgrade, completed_player_upgrades, radar_grant);
}
