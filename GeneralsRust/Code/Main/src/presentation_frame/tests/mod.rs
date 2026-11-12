//! Honesty / residual tests for the presentation snapshot.
//!
//! Split from the former monolithic `presentation_frame.rs` `mod tests`.

pub use super::*;
pub use crate::game_logic::{GameLogic, GameMode, KindOf, ObjectId, Player, Team, ThingTemplate};
pub use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
pub use glam::Vec3;

mod apply_honesty;
mod dual_tick_registry;
mod fow_own_team;
mod freeze_queries;
mod render_overlay;
mod runtime_heightmap;
mod shared_contract;
mod weapon_readiness;

mod shroud_ownership;

/// Retail CommandSet.ini slots (1-based) for the retail Objects these
/// presentation fixtures name. Each fixture authors the set into its world's
/// own catalog and names it from the template's `CommandSet` field
/// (ThingTemplate.cpp:132 m_commandSetString), as the retail Object INI does;
/// presentation never infers a set from a template name.
const RETAIL_COMMAND_SETS: &[(&str, &str, &[(usize, &str)])] = &[
    (
        "AmericaInfantryColonelBurton",
        "AmericaInfantryColonelBurtonCommandSet",
        &[(1, "Command_ColonelBurtonKnifeAttack"), (2, "Command_ColonelBurtonTimedDemoCharge"), (4, "Command_ColonelBurtonRemoteDemoCharge"), (6, "Command_ColonelBurtonDetonateCharges"), (11, "Command_AttackMove"), (13, "Command_Guard"), (14, "Command_Stop")],
    ),
    (
        "AmericaInfantryRanger",
        "AmericaInfantryRangerCommandSet",
        &[(1, "Command_AmericaRangerCaptureBuilding"), (2, "Command_AmericaRangerSwitchToMachineGun"), (4, "Command_AmericaRangerSwitchToFlagBangGrenades"), (11, "Command_AttackMove"), (13, "Command_Guard"), (14, "Command_Stop")],
    ),
    (
        "AmericaBarracks",
        "AmericaBarracksCommandSet",
        &[(1, "Command_ConstructAmericaInfantryRanger"), (7, "Command_UpgradeAmericaRangerFlashBangGrenade"), (8, "Command_UpgradeAmericaRangerCaptureBuilding"), (13, "Command_SetRallyPoint"), (14, "Command_Sell")],
    ),
    (
        "AmericaPowerPlant",
        "AmericaPowerPlantCommandSet",
        &[(1, "Command_UpgradeAmericaAdvancedControlRods"), (14, "Command_Sell")],
    ),
    (
        "AmericaVehicleChinook",
        "AmericaVehicleChinookCommandSet",
        &[(1, "Command_TransportExit"), (9, "Command_ChinookUnload"), (10, "Command_CombatDrop"), (14, "Command_Stop")],
    ),
    (
        "AmericaVehicleHumvee",
        "AmericaVehicleHumveeCommandSet",
        &[(4, "Command_TransportExit"), (9, "Command_Evacuate"), (11, "Command_AttackMove"), (13, "Command_Guard"), (14, "Command_Stop")],
    ),
    (
        "AmericaParticleCannonUplink",
        "AmericaParticleUplinkCannonCommandSet",
        &[(1, "Command_FireParticleUplinkCannon"), (14, "Command_Sell")],
    ),
    (
        "AmericaVehicleSentryDrone",
        "AmericaVehicleSentryDroneCommandSet",
        &[(11, "Command_AttackMove"), (13, "Command_Guard"), (14, "Command_Stop")],
    ),
];

fn author_retail_command_set(logic: &GameLogic, template: &mut ThingTemplate) {
    let (_, set_name, slots) = RETAIL_COMMAND_SETS
        .iter()
        .find(|(name, ..)| *name == template.name)
        .expect("retail CommandSet fixture for template");
    logic.edit_own_command_definitions_for_test(|_, manager| {
        let set = manager.new_command_set((*set_name).into());
        for (slot, command) in *slots {
            set.set_button_at_position(slot - 1, (*command).into())
                .expect("authored CommandSet slot");
        }
    });
    template.command_set_name = (*set_name).into();
}

/// Admit the same real Main objects used by presentation health-event controls.
fn admit_health_fixture_objects(logic: &mut GameLogic, count: u32) -> ObjectId {
    let mut template = ThingTemplate::new("PresentationHealthFixture");
    template.set_health(100.0);
    template.add_kind_of(KindOf::Infantry);
    logic
        .templates
        .insert("PresentationHealthFixture".into(), template);
    let mut target = ObjectId(0);
    for _ in 0..count {
        target = logic
            .create_object("PresentationHealthFixture", Team::USA, Vec3::ZERO)
            .expect("health fixture object");
    }
    assert_eq!(target.0, count);
    target
}
