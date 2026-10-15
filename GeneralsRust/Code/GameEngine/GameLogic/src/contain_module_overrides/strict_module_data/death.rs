//! Strict death module-data registrations.
use super::*;

pub(super) fn register(factory: &mut impl DataParserRegistrar) {
    register_parsed!(factory, Behavior, "DestroyDie", DieModuleData, default);
    register_parsed!(factory, Behavior, "KeepObjectDie", DieModuleData, default);
    register_parsed!(
        factory,
        Behavior,
        "UpgradeDie",
        UpgradeDieModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "CreateObjectDie",
        CreateObjectDieModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "CreateCrateDie",
        CreateCrateDieModuleData,
        default
    );
    register_parsed!(factory, Behavior, "FXListDie", FXListDieModuleData, default);
    register_parsed!(factory, Behavior, "CrushDie", CrushDieModuleData, default);
    register_parsed!(
        factory,
        Behavior,
        "EjectPilotDie",
        EjectPilotDieModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "RebuildHoleExposeDie",
        RebuildHoleExposeDieModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "SpecialPowerCompletionDie",
        SpecialPowerCompletionDieModuleData,
        default
    );
    register_parsed!(factory, Behavior, "DamDie", DamDieModuleData, default);
    register_parsed!(
        factory,
        Behavior,
        "HelicopterSlowDeathBehavior",
        HelicopterSlowDeathBehaviorModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "SlowDeathBehavior",
        SlowDeathBehaviorModuleData,
        new
    );
    register_parsed!(
        factory,
        Behavior,
        "PoisonedBehavior",
        PoisonedBehaviorModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "JetSlowDeathBehavior",
        JetSlowDeathBehaviorModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "InstantDeathBehavior",
        InstantDeathBehaviorModuleData,
        default
    );
}

#[cfg(test)]
pub(super) const NAMES: &[&str] = &[
    "DestroyDie",
    "KeepObjectDie",
    "UpgradeDie",
    "CreateObjectDie",
    "CreateCrateDie",
    "FXListDie",
    "CrushDie",
    "EjectPilotDie",
    "RebuildHoleExposeDie",
    "SpecialPowerCompletionDie",
    "DamDie",
    "HelicopterSlowDeathBehavior",
    "SlowDeathBehavior",
    "PoisonedBehavior",
    "JetSlowDeathBehavior",
    "InstantDeathBehavior",
];
