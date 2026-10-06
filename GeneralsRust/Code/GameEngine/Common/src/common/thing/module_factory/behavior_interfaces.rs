//! Immutable capabilities of the C++ ModuleFactory's registered Behavior classes.

/// Whether a known builtin exposes `BehaviorModule::getAIUpdateInterface`.
///
/// This is separate from the UPDATE mask: most UpdateModules have no AI
/// interface, while FlightDeckBehavior inherits it from AIUpdateInterface.
/// Unknown/custom classes return `None`; their names cannot prove absence.
/// No module, name key, global catalog, or runtime owner is constructed.
pub fn builtin_behavior_has_ai_update_interface(class_name: &str) -> Option<bool> {
    let known = super::BUILTIN_BEHAVIOR_DESCRIPTORS
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case(class_name));
    if !known {
        return None;
    }

    // ModuleFactory.cpp's addModule registrations and the corresponding
    // GameLogic/Module headers' AIUpdateInterface inheritance. Its virtual
    // getAIUpdateInterface (AIUpdate.h:279) returns this for every subclass.
    const AI_INTERFACES: &[&str] = &[
        "AIUpdateInterface",
        "AssaultTransportAIUpdate",
        "ChinookAIUpdate",
        "DeliverPayloadAIUpdate",
        "DeployStyleAIUpdate",
        "DozerAIUpdate",
        "FlightDeckBehavior",
        "HackInternetAIUpdate",
        "JetAIUpdate",
        "MissileAIUpdate",
        "POWTruckAIUpdate",
        "RailedTransportAIUpdate",
        "SupplyTruckAIUpdate",
        "TransportAIUpdate",
        "WanderAIUpdate",
        "WorkerAIUpdate",
    ];
    Some(
        AI_INTERFACES
            .iter()
            .any(|name| name.eq_ignore_ascii_case(class_name)),
    )
}
