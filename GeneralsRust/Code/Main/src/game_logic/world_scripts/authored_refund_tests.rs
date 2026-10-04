//! Actual Main Object INI projection -> producer admission -> paid queue -> cancel.
//! Common command catalogs are explicitly unavailable for these unique names;
//! this exercises the existing Main capability fallback, not retail authorization.
use super::super::*;
use crate::game_logic::game_logic::gameworld_authority::GameWorldAuthority;
use crate::game_logic::host_economy_log::{
    self, HostEconomyEvent, HostMoneyAudio, HostMoneyAudioEvent,
};
use crate::game_logic::{GameLogic, KindOf, ObjectId, Player, Resources, Team};
use crate::gameworld_shadow::{CoupledTickGuard, with_gameworld_authority};

const PRODUCER: &str = "AuthoredRefundOwnerBarracks";
const UNIT: &str = "AuthoredRefundOwnerInfantry";
const OWNER: u32 = 4;
const TEAMMATE: u32 = 0;

struct ClearEconomyLog;
impl Drop for ClearEconomyLog {
    fn drop(&mut self) {
        host_economy_log::clear();
    }
}

fn authored_world(cost: u32, bank: u32) -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    logic.set_current_frame(73);
    let mut owner = Player::new(OWNER, Team::USA, "RefundOwner", true);
    owner.resources.supplies = bank;
    owner.power_available = 40;
    logic.add_player(owner);
    let mut teammate = Player::new(TEAMMATE, Team::USA, "RefundTeammate", false);
    teammate.resources.supplies = 9_000;
    teammate.power_available = 91;
    logic.add_player(teammate);

    let source = format!(
        r#"
Object {PRODUCER}
  KindOf = STRUCTURE FS_BARRACKS SELECTABLE
  BuildCost = 600
  BuildTime = 30
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 1000
  End
  Behavior = ProductionUpdate ModuleTag_Production
    MaxQueueEntries = 9
  End
End
Object {UNIT}
  KindOf = INFANTRY SELECTABLE
  BuildCost = {cost}
  BuildTime = 30
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100
  End
End
"#
    );
    let mut parser = crate::assets::IniParser::new();
    assert_eq!(
        parser
            .parse_ini_content(&source, "authored_refund.ini")
            .unwrap(),
        2
    );
    for name in [PRODUCER, UNIT] {
        let definition = parser.get_definition(name).expect("actual parsed Object");
        let template = GameLogic::build_template_from_object_definition(name, definition, None);
        assert_eq!(template.build_time, 30.0);
        if name == UNIT {
            assert!(template.is_kind_of(KindOf::Infantry));
            assert_eq!(template.build_cost.supplies, cost);
            assert_eq!(template.build_cost.power, 0);
            assert_eq!(template.max_health, 100.0);
        } else {
            assert!(template.is_kind_of(KindOf::Structure));
            assert_eq!(template.max_health, 1000.0);
        }
        logic.templates.insert(name.to_string(), template);
    }
    assert_eq!(
        gamelogic::control_bar::parsed_unit_build_authorization(PRODUCER, UNIT),
        gamelogic::control_bar::ParsedUnitBuildAuthorization::Unavailable,
        "fixture must not silently claim an authored Common CommandSet gate"
    );
    let producer = logic
        .create_object_for_player(PRODUCER, OWNER, glam::Vec3::ZERO)
        .expect("actual admitted parsed producer");
    let object = logic.host_object(producer).unwrap();
    assert_eq!(object.owner_player_id, Some(OWNER));
    assert!(object.is_alive() && object.is_constructed());
    let building = object
        .building_data
        .as_ref()
        .expect("admission creates real producer data");
    assert!(building.production_queue.is_empty());
    assert!(building.can_produce(&logic.templates[UNIT]));
    assert_eq!(
        logic.can_make_unit(producer, UNIT),
        crate::game_logic::host_production_buildable_command_residual::CANMAKE_OK
    );
    assert_eq!(
        logic.get_player(TEAMMATE).unwrap().effective_supplies(),
        9_000
    );
    host_economy_log::clear();
    (logic, producer)
}

fn enqueue(logic: &mut GameLogic, producer: ObjectId, cost: u32) -> Vec<HostEconomyEvent> {
    let before = logic.get_player(OWNER).unwrap().effective_supplies();
    let power = logic.get_player(OWNER).unwrap().power_available;
    let before_queue = logic
        .host_object(producer)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap()
        .production_queue
        .len();
    assert!(logic.enqueue_production(producer, UNIT.to_string()));
    let building = logic
        .host_object(producer)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap();
    assert_eq!(building.production_queue.len(), before_queue + 1);
    let item = building.production_queue.last().unwrap();
    assert!(!item.is_upgrade());
    assert_eq!(item.template_name, UNIT);
    assert_eq!(item.cost.supplies, cost);
    assert_eq!(item.cost.power, 0);
    // The ordinary enqueue path preserves C++'s 900 integer logic frames
    // in Main's seconds carrier, just above the lower frame boundary.
    assert_eq!(item.total_time, (900.0_f32 + 0.25) / 30.0);
    assert_eq!(
        gamelogic::world::entities::production_total_logic_frames(
            item.total_time,
            item.is_upgrade(),
            1.0,
        ),
        900,
    );
    assert_eq!(item.construction_frames, 0);
    assert_eq!(item.quantity_total, 1);
    assert_eq!(item.quantity_produced, 0);
    assert_eq!(
        logic.get_player(OWNER).unwrap().effective_supplies(),
        before - cost
    );
    let events = host_economy_log::drain();
    assert_eq!(
        events,
        vec![HostEconomyEvent {
            player_id: OWNER,
            supplies: before - cost,
            power_available: power,
        }]
    );
    assert_eq!(
        host_economy_log::take_money_audio(),
        vec![HostMoneyAudioEvent {
            player_id: OWNER,
            kind: HostMoneyAudio::Withdraw,
        }]
    );
    events
}

fn assert_refund(logic: &GameLogic, expected_bank: u32) {
    let owner = logic.get_player(OWNER).unwrap();
    assert_eq!(owner.effective_supplies(), expected_bank);
    assert_eq!(
        host_economy_log::drain(),
        vec![HostEconomyEvent {
            player_id: OWNER,
            supplies: expected_bank,
            power_available: owner.power_available,
        }],
        "one refund publishes exactly one final cash-and-power observation"
    );
    assert_eq!(
        host_economy_log::take_money_audio(),
        vec![HostMoneyAudioEvent {
            player_id: OWNER,
            kind: HostMoneyAudio::Deposit,
        }],
        "CPP Money::deposit requests the controlling player's Deposit cue"
    );
    assert_eq!(
        logic.get_player(TEAMMATE).unwrap().effective_supplies(),
        9_000
    );
    assert_eq!(logic.get_player(TEAMMATE).unwrap().power_available, 91);
}

#[test]
fn authored_slot_and_name_cancel_refund_once_keep_clock_and_owner() {
    let _serial = crate::gameworld_shadow::authority_env_lock();
    let _cleanup = ClearEconomyLog;
    with_gameworld_authority(GameWorldAuthority::DEFAULT_OFF, || {
        let (mut logic, producer) = authored_world(250, 1_000);
        let _ = enqueue(&mut logic, producer, 250);
        let _ = enqueue(&mut logic, producer, 250);
        // Real host production admission advances only the head once at frame 73.
        logic.update_production(1.0 / 30.0);
        let queue = &logic
            .host_object(producer)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue;
        assert_eq!(queue[0].construction_frames, 1);
        assert_eq!(queue[1].construction_frames, 0);
        host_economy_log::clear();
        assert!(logic.cancel_production_at_index(producer, 1));
        assert_refund(&logic, 750);
        let queue = &logic
            .host_object(producer)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue;
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].construction_frames, 1);
        assert_eq!(queue[0].cost.supplies, 250);
        assert_eq!(logic.get_current_frame(), 73);
        assert!(logic.cancel_production(producer, UNIT.to_ascii_lowercase()));
        assert_refund(&logic, 1_000);
        assert!(!logic.cancel_production(producer, UNIT.to_string()));
        assert!(host_economy_log::drain().is_empty());
        assert!(host_economy_log::take_money_audio().is_empty());
    });
}

#[test]
fn authored_same_id_worlds_refund_their_own_paid_costs() {
    let _serial = crate::gameworld_shadow::authority_env_lock();
    let _cleanup = ClearEconomyLog;
    with_gameworld_authority(GameWorldAuthority::DEFAULT_OFF, || {
        let (mut first, first_producer) = authored_world(250, 1_000);
        let _ = enqueue(&mut first, first_producer, 250);
        let (mut second, second_producer) = authored_world(375, 2_000);
        assert_eq!(first_producer, second_producer);
        let _ = enqueue(&mut second, second_producer, 375);
        assert_eq!(first.get_player(OWNER).unwrap().effective_supplies(), 750);
        assert!(first.cancel_production(first_producer, UNIT.to_string()));
        assert_refund(&first, 1_000);
        assert_eq!(
            second.get_player(OWNER).unwrap().effective_supplies(),
            1_625
        );
        assert_eq!(
            second
                .host_object(second_producer)
                .unwrap()
                .building_data
                .as_ref()
                .unwrap()
                .production_queue[0]
                .cost
                .supplies,
            375
        );
        assert!(second.cancel_production_at_index(second_producer, 0));
        assert_refund(&second, 2_000);
        assert_eq!(first.get_player(OWNER).unwrap().effective_supplies(), 1_000);
        assert!(
            first
                .host_object(first_producer)
                .unwrap()
                .building_data
                .as_ref()
                .unwrap()
                .production_queue
                .is_empty()
        );
    });
}

#[test]
fn authored_cancel_all_has_one_final_event_and_deposit_per_paid_entry() {
    let _serial = crate::gameworld_shadow::authority_env_lock();
    let _cleanup = ClearEconomyLog;
    with_gameworld_authority(GameWorldAuthority::DEFAULT_OFF, || {
        let (mut logic, producer) = authored_world(250, 1_000);
        let _ = enqueue(&mut logic, producer, 250);
        let _ = enqueue(&mut logic, producer, 250);
        assert!(logic.cancel_all_production(producer));
        let power = logic.get_player(OWNER).unwrap().power_available;
        assert_eq!(
            host_economy_log::drain(),
            vec![
                HostEconomyEvent {
                    player_id: OWNER,
                    supplies: 750,
                    power_available: power
                },
                HostEconomyEvent {
                    player_id: OWNER,
                    supplies: 1_000,
                    power_available: power
                },
            ]
        );
        assert_eq!(
            host_economy_log::take_money_audio(),
            vec![
                HostMoneyAudioEvent {
                    player_id: OWNER,
                    kind: HostMoneyAudio::Deposit
                },
                HostMoneyAudioEvent {
                    player_id: OWNER,
                    kind: HostMoneyAudio::Deposit
                },
            ]
        );
        assert_eq!(logic.get_player(OWNER).unwrap().effective_supplies(), 1_000);
        assert!(!logic.cancel_all_production(producer));
        assert!(host_economy_log::drain().is_empty());
        assert!(host_economy_log::take_money_audio().is_empty());
    });
}

#[test]
fn authored_refund_coupled_economy_keeps_base_and_delta_contract() {
    let _serial = crate::gameworld_shadow::authority_env_lock();
    let _cleanup = ClearEconomyLog;
    let authority = GameWorldAuthority {
        economy: true,
        ..GameWorldAuthority::DEFAULT_OFF
    };
    with_gameworld_authority(authority, || {
        let _coupled = CoupledTickGuard::enter();
        assert!(
            crate::gameworld_shadow::gameworld_economy_authority_live(),
            "this control requires the actual coupled economy declaration"
        );
        let (mut logic, producer) = authored_world(250, 1_000);
        logic.set_economy_authority(true);
        let mut shadow = crate::gameworld_shadow::GameWorldShadow::new(8);
        shadow.sync_from_host(&logic);
        let withdrawal = enqueue(&mut logic, producer, 250);
        assert_eq!(shadow.apply_host_economy_events(&withdrawal), (2, 2));
        // sync_players allocates these exact host IDs in sorted order: 0, 4.
        let shadow_owner = gamelogic::world::PlayerId::from_index(1);
        let shadow_teammate = gamelogic::world::PlayerId::from_index(0);
        assert_eq!(shadow.world().player(shadow_owner).unwrap().supplies, 750);
        let player = logic.get_player(OWNER).unwrap();
        assert_eq!(player.resources.supplies, 1_000);
        assert_eq!(player.pending_supply_delta, -250);
        assert!(logic.cancel_production_at_index(producer, 0));
        let owner = logic.get_player(OWNER).unwrap();
        let refund = host_economy_log::drain();
        assert_eq!(
            refund,
            vec![HostEconomyEvent {
                player_id: OWNER,
                supplies: 1_000,
                power_available: owner.power_available,
            }]
        );
        assert_eq!(
            host_economy_log::take_money_audio(),
            vec![HostMoneyAudioEvent {
                player_id: OWNER,
                kind: HostMoneyAudio::Deposit,
            }]
        );
        assert_eq!(
            shadow.apply_host_economy_events(&refund),
            (2, 2),
            "one refund applies one SetSupplies + SetPower pair"
        );
        assert_eq!(shadow.world().player(shadow_owner).unwrap().supplies, 1_000);
        assert_eq!(
            shadow.world().player(shadow_teammate).unwrap().supplies,
            9_000
        );
        let _ = shadow.writeback_economy_to_host(&mut logic);
        let _ = crate::game_logic::host_economy_ready_log::drain();
        let player = logic.get_player(OWNER).unwrap();
        assert_eq!(player.resources.supplies, 1_000);
        assert_eq!(player.pending_supply_delta, 0);
    });
}

#[test]
fn parsed_upgrade_ledger_cancel_refunds_once() {
    let _serial = crate::gameworld_shadow::authority_env_lock();
    let _cleanup = ClearEconomyLog;
    with_gameworld_authority(GameWorldAuthority::DEFAULT_OFF, || {
        let (mut logic, _) = authored_world(250, 1_000);
        let source =
            "Upgrade_AuthoredRefundLedger\nType = PLAYER\nBuildCost = 321\nBuildTime = 9\nEnd\n";
        let mut ini = game_engine::common::ini::INI::new();
        ini.with_inline_source(source, |ini| {
            ini.read_line()?;
            logic
                .engine_stores
                .upgrade_center()
                .write()
                .unwrap()
                .parse_upgrade_definition(ini)
                .map_err(|_| game_engine::common::ini::INIError::InvalidData)
        })
        .expect("real owned UpgradeCenter parses authored rules");
        let definition = logic
            .upgrade_template("Upgrade_AuthoredRefundLedger")
            .unwrap();
        assert_eq!(definition.get_cost(), 321);
        assert_eq!(definition.get_build_time(), 9.0);
        assert_eq!(
            definition.get_upgrade_type(),
            gamelogic::upgrade::UpgradeType::Player
        );
        let cost = Resources {
            supplies: definition.get_cost() as u32,
            power: 0,
        };
        let player = logic.get_player_mut(OWNER).unwrap();
        let spent_before = player.statistics.resources_spent;
        let income_before = player.statistics.resources_collected;
        assert!(player.queue_upgrade(
            "Upgrade_AuthoredRefundLedger",
            &cost,
            definition.get_upgrade_type()
        ));
        assert!(player.has_queued_upgrade("Upgrade_AuthoredRefundLedger"));
        assert_eq!(player.statistics.resources_spent, spent_before + 321);
        host_economy_log::clear();
        assert!(player.cancel_queued_upgrade("upgrade_authoredrefundledger", &cost));
        assert!(!player.has_queued_upgrade("Upgrade_AuthoredRefundLedger"));
        assert_eq!(
            player.statistics.resources_collected, income_before,
            "refund must not acquire supply-income semantics as an accounting shortcut"
        );
        assert_refund(&logic, 1_000);
        let player = logic.get_player_mut(OWNER).unwrap();
        assert!(!player.cancel_queued_upgrade("Upgrade_AuthoredRefundLedger", &cost));
        assert!(host_economy_log::drain().is_empty());
        assert!(host_economy_log::take_money_audio().is_empty());
    });
}

#[test]
fn zero_money_ledger_cancel_reverses_power_without_deposit_cue() {
    let _serial = crate::gameworld_shadow::authority_env_lock();
    let _cleanup = ClearEconomyLog;
    with_gameworld_authority(GameWorldAuthority::DEFAULT_OFF, || {
        let (mut logic, _) = authored_world(250, 1_000);
        let player = logic.get_player_mut(OWNER).unwrap();
        // Existing public Player ledger edge: zero money still reverses power.
        // This is a ledger boundary control, not an authored producer admission.
        let zero = Resources {
            supplies: 0,
            power: -7,
        };
        let before_power = player.power_available;
        assert!(player.queue_upgrade(
            "Upgrade_ZeroMoneyLedgerControl",
            &zero,
            gamelogic::upgrade::UpgradeType::Player
        ));
        assert_eq!(player.power_available, before_power - 7);
        host_economy_log::clear();
        assert!(player.cancel_queued_upgrade("Upgrade_ZeroMoneyLedgerControl", &zero));
        assert_eq!(player.power_available, before_power);
        assert_eq!(
            host_economy_log::drain(),
            vec![HostEconomyEvent {
                player_id: OWNER,
                supplies: 1_000,
                power_available: before_power,
            }]
        );
        assert!(host_economy_log::take_money_audio().is_empty());
    });
}
