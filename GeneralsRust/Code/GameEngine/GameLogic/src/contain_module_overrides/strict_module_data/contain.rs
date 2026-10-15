//! Strict contain module-data registrations.
use super::*;

pub(super) fn register(factory: &mut impl DataParserRegistrar) {
    register_contain!(factory, "OpenContain", OpenContainModuleData);
    register_contain!(factory, "TransportContain", TransportContainModuleData);
    register_contain!(factory, "GarrisonContain", GarrisonContainModuleData);
    register_contain!(factory, "TunnelContain", TunnelContainModuleData);
    register_contain!(factory, "OverlordContain", OverlordContainModuleData);
    register_contain!(factory, "HelixContain", HelixContainModuleData);
    register_contain!(factory, "ParachuteContain", ParachuteContainModuleData);
    register_contain!(factory, "MobNexusContain", MobNexusContainModuleData);
    register_contain!(
        factory,
        "RailedTransportContain",
        RailedTransportContainModuleData
    );
    register_contain!(factory, "RiderChangeContain", RiderChangeContainModuleData);
    register_contain!(
        factory,
        "InternetHackContain",
        InternetHackContainModuleData
    );
    register_contain!(factory, "HealContain", HealContainModuleData);
    register_contain!(factory, "CaveContain", CaveContainModuleData);
}

#[cfg(test)]
pub(super) const NAMES: &[&str] = &[
    "OpenContain",
    "TransportContain",
    "GarrisonContain",
    "TunnelContain",
    "OverlordContain",
    "HelixContain",
    "ParachuteContain",
    "MobNexusContain",
    "RailedTransportContain",
    "RiderChangeContain",
    "InternetHackContain",
    "HealContain",
    "CaveContain",
];
