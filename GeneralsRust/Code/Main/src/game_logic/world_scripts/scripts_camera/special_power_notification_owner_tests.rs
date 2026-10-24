//! Actual receiving notifications: ordinary command/update, borrowed Body/Die,
//! and nested private temporary Weapon visits. No test publishes an active owner.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
use gamelogic::scripting::engine::{ScriptEngine, ScriptExecutionDriver};
const POWER: &str = "SpecialPowerSpySatellite";
const CREATOR: u32 = 42;
const BLAST: &str = "ReceivingNotificationBlast";
fn world(spawn: bool) -> (GameLogic, Option<ObjectId>, Option<ObjectId>) {
    let mut world = GameLogic::new();
    world.world_min = Vec3::ZERO;
    world.world_max = Vec3::splat(1000.0);
    let source = format!(
        "Object NotificationCarrier\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = FireWeaponWhenDeadBehavior BlastTag\nStartsActive = Yes\nDeathWeapon = {BLAST}\nEnd\nBehavior = SpecialPowerCompletionDie CarrierCompletionTag\nSpecialPowerTemplate = {POWER}\nEnd\nBehavior = KeepObjectDie KeepTag\nEnd\nEnd\nObject NotificationVictim\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = SpecialPowerCompletionDie CompletionTag\nSpecialPowerTemplate = {POWER}\nEnd\nBehavior = KeepObjectDie KeepTag\nEnd\nEnd\n"
    );
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "receiving-notification.ini".into(),
                source,
            )])
            .unwrap(),
        )
        .unwrap();
    let mut definitions = HashMap::new();
    assert_eq!(
        crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
            &mut definitions,
            &format!(
                "Weapon {BLAST}\nPrimaryDamage = 200\nPrimaryDamageRadius = 100\nDamageType = EXPLOSION\nRadiusDamageAffects = ALLIES ENEMIES NEUTRALS\nProjectileObject = NONE\nClipSize = 1\nClipReloadTime = 0\nDelayBetweenShots = 0\nEnd\n"
            ),
            crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
        ),
        1
    );
    world
        .admit_weapon_content_snapshot(
            definitions
                .into_values()
                .map(|rule| (rule.name.clone(), rule))
                .collect(),
            &[],
        )
        .unwrap();
    // Same-faction player1 is deliberately first; onDie belongs to exact player2.
    world.add_player(Player::new(1, Team::USA, "FirstAlly", true));
    world.add_player(
        Player::new(2, Team::USA, "EventOwner", false).with_script_identity("EventOwner"),
    );
    world.install_owned_script_engine(ScriptEngine::new().unwrap());
    if !spawn {
        return (world, None, None);
    }
    let carrier = world
        .create_object_for_player("NotificationCarrier", 2, Vec3::new(30., 0., 30.))
        .unwrap();
    let victim = world
        .create_object_for_player("NotificationVictim", 2, Vec3::new(35., 0., 30.))
        .unwrap();
    world.host_object_mut(carrier).unwrap().name = "NotificationCarrier".into();
    world
        .host_object_mut(carrier)
        .unwrap()
        .set_special_power_completion(POWER, CREATOR + 1);
    world.host_object_mut(victim).unwrap().name = "NotificationVictim".into();
    world
        .host_object_mut(victim)
        .unwrap()
        .set_special_power_completion(POWER, CREATOR);
    world.update_owned_partition_with_effects(&mut Default::default());
    assert_eq!(
        world.host_object_id_by_script_name("NotificationCarrier"),
        None
    );
    assert_eq!(
        world.host_object_id_by_script_name("NotificationVictim"),
        None
    );
    // CPP ScriptEngine first UPDATE seeds the initial named request vector.
    // A direct friend_execute_action is not that phase; exercise the actual
    // receiving update/driver before any test asks NamedKill to resolve names.
    world.scripts_loaded = true;
    world
        .with_owned_script_engine(|engine, receiving| {
            let context = gamelogic::scripting::executor::ScriptContext::at_frame(receiving.frame);
            engine.update_with_driver(
                context,
                &mut HostScriptExecutionDriver::with_interpreter(receiving, engine),
            )
        })
        .expect("fixture owns its receiving interpreter")
        .expect("actual first script update");
    assert_eq!(
        world.host_object_id_by_script_name("NotificationCarrier"),
        Some(carrier)
    );
    assert_eq!(
        world.host_object_id_by_script_name("NotificationVictim"),
        Some(victim)
    );
    assert_eq!(world.special_power_completion_log.notifications, 0);
    (world, Some(carrier), Some(victim))
}
fn action(name: &str) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::NamedKill);
    action
        .add_parameter(Parameter::with_string(ParameterType::Unit, name.into()))
        .unwrap();
    action
}
fn completed(engine: &ScriptEngine, source: u32) -> bool {
    engine.is_special_power_complete(2, POWER, false, source)
}
// Enter a foreign Core interpreter by its actual public execution ingress.
// Its callback invokes the already supplied receiving operation; this detects
// accidental TLS selection without changing process-global test stores.
struct Visit<F: FnMut()> {
    visit: F,
}
impl<F: FnMut()> ScriptExecutionDriver for Visit<F> {
    fn after_action(&mut self) -> gamelogic::GameLogicResult<()> {
        Ok(())
    }
    fn delete_or_kill(
        &mut self,
        _: &gamelogic::scripting::HostScriptKillDeleteDamageRequest,
        _: Option<gamelogic::common::ObjectID>,
        _: Option<gamelogic::team::TeamID>,
        _: Option<gamelogic::team::TeamID>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        (self.visit)();
        Some(Ok(()))
    }
}
fn under_foreign(engine: &ScriptEngine, visit: impl FnMut()) {
    engine.friend_execute_action_with_driver(
        &action("ForeignSentinel"),
        None,
        gamelogic::scripting::executor::ScriptContext::at_frame(11),
        &mut Visit { visit },
    );
}
#[test]
fn ordinary_special_power_producer_uses_receiving_interpreter_not_foreign_core() {
    let (mut owner, carrier, _) = world(true);
    let (peer, peer_carrier, _) = world(true);
    let carrier = carrier.unwrap();
    assert_eq!(Some(carrier), peer_carrier);
    let foreign = ScriptEngine::new().unwrap();
    foreign.notify_of_triggered_special_power(2, "ForeignSentinel", carrier.0);
    under_foreign(&foreign, || {
        assert!(
            owner
                .queue_special_power_strike(
                    &crate::command_system::SpecialPowerType::Airstrike,
                    carrier,
                    Vec3::new(200., 0., 200.)
                )
                .is_some()
        );
    });
    let name = "SuperweaponA10ThunderboltMissileStrike";
    assert!(
        owner
            .owned_script_engine
            .as_ref()
            .unwrap()
            .is_special_power_triggered(2, name, false, carrier.0)
    );
    assert!(
        !peer
            .owned_script_engine
            .as_ref()
            .unwrap()
            .is_special_power_triggered(2, name, false, carrier.0)
    );
    assert!(!foreign.is_special_power_triggered(2, name, false, carrier.0));
    assert!(foreign.is_special_power_triggered(2, "ForeignSentinel", false, carrier.0));
}
#[test]
fn borrowed_actual_driver_completion_die_notifies_before_next_action_and_stays_in_owner() {
    let (mut owner, _, victim) = world(true);
    let (peer, _, peer_victim) = world(true);
    assert_eq!(victim, peer_victim);
    let foreign = ScriptEngine::new().unwrap();
    foreign.notify_of_completed_special_power(2, POWER, 777);
    owner
        .with_owned_script_engine(|engine, world| {
            assert!(world.owned_script_engine.is_none());
            engine.friend_execute_action_with_driver(
                &action("NotificationVictim"),
                None,
                gamelogic::scripting::executor::ScriptContext::at_frame(11),
                &mut HostScriptExecutionDriver::with_interpreter(world, engine),
            );
            assert!(
                completed(engine, CREATOR),
                "onDie event is immediate, without a frame drain"
            );
            assert!(!engine.is_special_power_complete(1, POWER, false, CREATOR));
        })
        .unwrap();
    assert_eq!(owner.special_power_completion_log.notifications, 1);
    assert!(!completed(
        peer.owned_script_engine.as_ref().unwrap(),
        CREATOR
    ));
    assert!(peer.host_object(peer_victim.unwrap()).unwrap().is_alive());
    assert!(!completed(&foreign, CREATOR));
    assert!(completed(&foreign, 777));
}
#[test]
fn borrowed_body_death_weapon_reborrows_same_notification_recipient_into_nested_on_die() {
    let (mut owner, carrier, victim) = world(true);
    let (peer, peer_carrier, peer_victim) = world(true);
    assert_eq!(carrier, peer_carrier);
    assert_eq!(victim, peer_victim);
    let foreign = ScriptEngine::new().unwrap();
    owner
        .with_owned_script_engine(|engine, world| {
            under_foreign(&foreign, || {
                world.host_script_damage_with_interpreter_with_effects(
                    carrier.unwrap(),
                    0.,
                    true,
                    Some(engine),
                    &mut Default::default(),
                );
            });
            assert!(
                completed(engine, CREATOR),
                "private onDie Weapon -> splash -> Body -> onDie must keep the same short loan"
            );
            assert!(
                world
                    .host_object(victim.unwrap())
                    .unwrap()
                    .status
                    .on_die_started
            );
            assert_eq!(world.special_power_completion_log.notifications, 2);
            assert_eq!(
                engine.snapshot_xfer_tail().finished_special_powers[2],
                vec![(POWER.to_owned(), CREATOR), (POWER.to_owned(), CREATOR + 1)],
                "nested child completion precedes the parent's later authored CompletionDie slot"
            );
        })
        .unwrap();
    assert!(!completed(&foreign, CREATOR));
    assert!(!completed(
        peer.owned_script_engine.as_ref().unwrap(),
        CREATOR
    ));
    assert!(peer.host_object(peer_victim.unwrap()).unwrap().is_alive());
}
#[test]
fn special_power_event_encoded_continuation_preserves_receiving_history_and_flush() {
    let (mut owner, _, victim) = world(true);
    owner.host_script_damage_with_interpreter_with_effects(
        victim.unwrap(),
        0.,
        true,
        None,
        &mut Default::default(),
    );
    assert!(completed(
        owner.owned_script_engine.as_ref().unwrap(),
        CREATOR
    ));
    let builder = crate::save_load::SnapshotBuilder::new();
    let captured = builder.create_world_snapshot(&owner).unwrap();
    let bytes = bincode_legacy::serialize(&captured).unwrap();
    let decoded = crate::save_load::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut receiving, _, _) = world(false);
    builder
        .restore_from_snapshot(&decoded, &mut receiving)
        .unwrap();
    assert!(completed(
        receiving.owned_script_engine.as_ref().unwrap(),
        CREATOR
    ));
    assert!(
        receiving
            .owned_script_engine
            .as_ref()
            .unwrap()
            .is_special_power_complete(2, POWER, true, CREATOR)
    );
    assert!(!completed(
        receiving.owned_script_engine.as_ref().unwrap(),
        CREATOR
    ));
    assert!(completed(
        owner.owned_script_engine.as_ref().unwrap(),
        CREATOR
    ));
    assert_eq!(owner.special_power_completion_log.notifications, 1);
}
#[test]
fn direct_delete_and_invalid_creator_do_not_manufacture_completion_events() {
    let (mut deleted, _, victim) = world(true);
    deleted.destroy_object(victim.unwrap());
    assert!(!completed(
        deleted.owned_script_engine.as_ref().unwrap(),
        CREATOR
    ));
    assert_eq!(deleted.special_power_completion_log.notifications, 0);
    let (mut invalid, _, victim) = world(true);
    // Build a new actor's initial creator receipt as INVALID, not a second
    // setCreator call (the original assignment is one-shot).
    invalid.host_object_mut(victim.unwrap()).unwrap().special_power_completion = Some(
        crate::game_logic::host_special_power_completion_die::HostSpecialPowerCompletionDieData::new(POWER,0));
    invalid.host_script_damage_with_interpreter_with_effects(
        victim.unwrap(),
        0.,
        true,
        None,
        &mut Default::default(),
    );
    assert!(!completed(invalid.owned_script_engine.as_ref().unwrap(), 0));
    assert_eq!(invalid.special_power_completion_log.notifications, 0);
}
