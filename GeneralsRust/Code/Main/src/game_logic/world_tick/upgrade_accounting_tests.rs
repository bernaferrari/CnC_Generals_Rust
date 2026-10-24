//! Receiving research score and Academy continuation at the production boundary.
use super::*;
use crate::game_logic::host_upgrade_rules::register_test_upgrade;
use crate::game_logic::{Player, Resources, Team, ThingTemplate};
use crate::save_load::snapshot::SnapshotBuilder;

fn world(cost: u32, kind: &str) -> (GameLogic, crate::game_logic::ObjectId) {
    let mut world = GameLogic::new();
    register_test_upgrade(&world, "Upgrade_OwnedProductionAccounting", kind, cost, 1);
    let mut first = Player::new(0, Team::USA, "owner", true);
    first.resources.supplies = 5000;
    world.add_player(first);
    world.add_player(Player::new(1, Team::USA, "same faction", false));
    let mut template = ThingTemplate::new("ReceivingAccountingProducer");
    template.set_health(100.);
    world.templates.insert(template.name.clone(), template);
    let producer = world
        .create_object_for_player("ReceivingAccountingProducer", 0, glam::Vec3::ZERO)
        .unwrap();
    assert!(
        world
            .host_object(producer)
            .unwrap()
            .team_instance_id
            .is_some()
    );
    assert_eq!(world.ordered_upgrade_owner_object_ids(0), [producer]);
    (world, producer)
}

#[test]
fn production_completion_accounts_only_receiving_owner_and_keeps_object_upgrade_off_player() {
    for kind in ["PLAYER", "OBJECT"] {
        let (mut owner, producer) = world(800, kind);
        let (peer, _) = world(350, kind);
        let upgrade_type = owner.upgrade_type("Upgrade_OwnedProductionAccounting");
        let cost = Resources {
            supplies: 800,
            power: 0,
        };
        assert!(owner.get_player_mut(0).unwrap().queue_upgrade(
            "Upgrade_OwnedProductionAccounting",
            &cost,
            upgrade_type,
            &mut Default::default()
        ));
        assert_eq!(owner.get_player(0).unwrap().statistics.resources_spent(), 0);
        owner.host_apply_upgrade_production_completions(vec![(
            Team::USA,
            "Upgrade_OwnedProductionAccounting".into(),
            producer,
        )]);
        let p = owner.get_player(0).unwrap();
        assert_eq!(p.resources.supplies, 4200);
        assert_eq!(p.statistics.resources_spent(), 800);
        assert_eq!(p.academy_stats.get_upgrades_purchased(), 1);
        assert_eq!(
            p.has_unlocked_upgrade("Upgrade_OwnedProductionAccounting"),
            kind == "PLAYER"
        );
        assert_eq!(owner.get_player(1).unwrap().statistics.resources_spent(), 0);
        assert_eq!(
            owner
                .get_player(1)
                .unwrap()
                .academy_stats
                .get_upgrades_purchased(),
            0
        );
        assert_eq!(peer.get_player(0).unwrap().statistics.resources_spent(), 0);
        assert_eq!(
            peer.get_player(0)
                .unwrap()
                .academy_stats
                .get_upgrades_purchased(),
            0
        );
    }
}

#[test]
fn completed_upgrade_score_and_academy_survive_encoded_save_and_future_completion() {
    let (mut source, producer) = world(800, "OBJECT");
    assert!(source.get_player_mut(0).unwrap().queue_upgrade(
        "Upgrade_OwnedProductionAccounting",
        &Resources {
            supplies: 800,
            power: 0
        },
        gamelogic::upgrade::UpgradeType::Object,
        &mut Default::default()
    ));
    source.host_apply_upgrade_production_completions(vec![(
        Team::USA,
        "Upgrade_OwnedProductionAccounting".into(),
        producer,
    )]);
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let snapshot = bincode_legacy::deserialize(&bytes).unwrap();
    let (mut restored, _) = world(800, "OBJECT");
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(
        restored.get_player(0).unwrap().statistics.resources_spent(),
        800
    );
    assert_eq!(
        restored
            .get_player(0)
            .unwrap()
            .academy_stats
            .get_upgrades_purchased(),
        1
    );
    assert!(restored.get_player_mut(0).unwrap().queue_upgrade(
        "Upgrade_OwnedProductionAccounting",
        &Resources {
            supplies: 800,
            power: 0
        },
        gamelogic::upgrade::UpgradeType::Object,
        &mut Default::default()
    ));
    restored.host_apply_upgrade_production_completions(vec![(
        Team::USA,
        "Upgrade_OwnedProductionAccounting".into(),
        producer,
    )]);
    assert_eq!(restored.get_player(0).unwrap().resources.supplies, 3400);
    assert_eq!(
        restored.get_player(0).unwrap().statistics.resources_spent(),
        1600
    );
    assert_eq!(
        restored
            .get_player(0)
            .unwrap()
            .academy_stats
            .get_upgrades_purchased(),
        2
    );
    assert_eq!(
        source.get_player(0).unwrap().statistics.resources_spent(),
        800
    );
    assert_eq!(
        source
            .get_player(0)
            .unwrap()
            .academy_stats
            .get_upgrades_purchased(),
        1
    );
}

#[test]
fn production_upgrade_and_encoded_restore_never_publish_into_foreign_core_player() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "production_upgrade_and_encoded_restore_never_publish_into_foreign_core_player",
        || {
            use std::sync::{Arc, RwLock};
            struct Restore(Option<gamelogic::player::PlayerList>);
            impl Drop for Restore {
                fn drop(&mut self) {
                    *gamelogic::player::player_list().write().unwrap() = self.0.take().unwrap();
                }
            }
            let mut foreign = gamelogic::player::Player::new(0);
            foreign.set_player_name_key(
                game_engine::common::name_key_generator::NameKeyGenerator::name_to_key("owner"),
            );
            foreign.set_display_name("owner");
            let foreign = Arc::new(RwLock::new(foreign));
            let mut roster = gamelogic::player::PlayerList::new();
            roster.add_player(Arc::clone(&foreign));
            let _restore = Restore(Some(std::mem::replace(
                &mut *gamelogic::player::player_list().write().unwrap(),
                roster,
            )));
            let (mut owner, producer) = world(800, "PLAYER");
            let (mut peer, peer_producer) = world(350, "PLAYER");
            assert_eq!(producer, peer_producer);
            let name = "Upgrade_OwnedProductionAccounting";
            let definition = owner.upgrade_template(name).unwrap();
            assert!(!foreign.read().unwrap().has_upgrade_complete(&definition));
            assert!(owner.get_player_mut(0).unwrap().queue_upgrade(
                name,
                &Resources {
                    supplies: 800,
                    power: 0
                },
                gamelogic::upgrade::UpgradeType::Player,
                &mut Default::default(),
            ));
            owner.host_apply_upgrade_production_completions(vec![(
                Team::USA,
                name.into(),
                producer,
            )]);
            assert!(owner.get_player(0).unwrap().has_unlocked_upgrade(name));
            assert_eq!(
                owner.get_player(0).unwrap().statistics.resources_spent(),
                800
            );
            assert_eq!(
                owner
                    .get_player(0)
                    .unwrap()
                    .academy_stats
                    .get_upgrades_purchased(),
                1
            );
            assert!(!peer.get_player(0).unwrap().has_unlocked_upgrade(name));
            assert!(
                !foreign.read().unwrap().has_upgrade_complete(&definition),
                "Main research must not complete a same-ID/same-name Core peer"
            );
            assert_eq!(
                foreign
                    .read()
                    .unwrap()
                    .get_academy_stats()
                    .get_upgrades_purchased(),
                0
            );
            let builder = SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&owner).unwrap();
            let bytes = bincode_legacy::serialize(&snapshot).unwrap();
            let snapshot = bincode_legacy::deserialize(&bytes).unwrap();
            let (mut restored, _) = world(800, "PLAYER");
            builder
                .restore_from_snapshot(&snapshot, &mut restored)
                .unwrap();
            assert!(restored.get_player(0).unwrap().has_unlocked_upgrade(name));
            assert!(
                !foreign.read().unwrap().has_upgrade_complete(&definition),
                "encoded continuation must restore only the receiving ledger"
            );
            peer.reset();
            drop(peer);
            assert!(owner.get_player(0).unwrap().has_unlocked_upgrade(name));
            assert!(restored.get_player(0).unwrap().has_unlocked_upgrade(name));
            assert!(!foreign.read().unwrap().has_upgrade_complete(&definition));
        },
    );
}

#[test]
fn actual_producer_tick_completes_receiving_research_and_retires_only_its_queue() {
    use crate::game_logic::buildings::{BuildingData, BuildingType};
    let (mut owner, producer) = world(800, "PLAYER");
    let (mut peer, peer_producer) = world(350, "PLAYER");
    assert_eq!(producer, peer_producer);
    let name = "Upgrade_OwnedProductionAccounting";
    for (world, cost) in [(&mut owner, 800), (&mut peer, 350)] {
        assert!(world.get_player_mut(0).unwrap().queue_upgrade(
            name,
            &Resources {
                supplies: cost,
                power: 0
            },
            gamelogic::upgrade::UpgradeType::Player,
            &mut Default::default(),
        ));
        let mut building = BuildingData::new(BuildingType::WarFactory);
        assert!(building.add_upgrade_to_queue(
            name.into(),
            0.0,
            Resources {
                supplies: cost,
                power: 0
            }
        ));
        world.host_object_mut(producer).unwrap().building_data = Some(building);
    }
    owner.update_production(1.0 / 30.0);
    assert!(
        owner
            .host_object(producer)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty()
    );
    assert_eq!(
        peer.host_object(peer_producer)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .len(),
        1
    );
    assert!(owner.get_player(0).unwrap().has_unlocked_upgrade(name));
    assert!(!peer.get_player(0).unwrap().has_unlocked_upgrade(name));
    assert_eq!(
        owner.get_player(0).unwrap().statistics.resources_spent(),
        800
    );
    assert_eq!(
        owner
            .get_player(0)
            .unwrap()
            .academy_stats
            .get_upgrades_purchased(),
        1
    );
    assert_eq!(peer.get_player(0).unwrap().statistics.resources_spent(), 0);
    owner.update_production(1.0 / 30.0);
    assert_eq!(
        owner.get_player(0).unwrap().statistics.resources_spent(),
        800
    );
    assert_eq!(
        owner
            .get_player(0)
            .unwrap()
            .academy_stats
            .get_upgrades_purchased(),
        1
    );
}

#[test]
fn actual_completion_transaction_keeps_entry_through_callbacks_and_preserves_callback_requeue() {
    use crate::game_logic::buildings::{BuildingData, BuildingType};
    let (mut owner, producer) = world(800, "PLAYER");
    let name = "Upgrade_OwnedProductionAccounting";
    let cost = Resources {
        supplies: 800,
        power: 0,
    };
    assert!(owner.get_player_mut(0).unwrap().queue_upgrade(
        name,
        &cost,
        gamelogic::upgrade::UpgradeType::Player,
        &mut Default::default(),
    ));
    let mut building = BuildingData::new(BuildingType::WarFactory);
    assert!(building.add_upgrade_to_queue(name.into(), 0.0, cost));
    let (_, entry_identity) = building.ready_upgrade_entry(1.0).unwrap();
    owner.host_object_mut(producer).unwrap().building_data = Some(building);
    let retained = RetainedUpgradeEntry {
        entry_identity,
        producer_identity: std::sync::Arc::clone(
            &owner
                .host_object(producer)
                .unwrap()
                .visual_draw_identity_token,
        ),
    };
    let mut observed = false;
    owner.complete_upgrade_on_owner_with_observer(
        None,
        Team::USA,
        name.into(),
        producer,
        Some(retained),
        &mut Default::default(),
        |world| {
            let player = world.get_player(0).unwrap();
            assert_eq!(player.statistics.resources_spent(), 800);
            assert_eq!(player.academy_stats.get_upgrades_purchased(), 1);
            assert!(player.has_unlocked_upgrade(name));
            let building = world
                .host_object_mut(producer)
                .unwrap()
                .building_data
                .as_mut()
                .unwrap();
            assert_eq!(
                building.production_queue.len(),
                1,
                "CPP retains entry through module/Academy/UI callbacks"
            );
            assert_eq!(building.production_queue[0].quantity_produced, 0);
            assert_eq!(
                building
                    .cancel_production(0)
                    .unwrap()
                    .runtime_entry_identity,
                entry_identity
            );
            assert!(building.add_upgrade_to_queue(name.into(), 0.0, cost));
            assert_ne!(
                building.production_queue[0].runtime_entry_identity,
                entry_identity
            );
            observed = true;
        },
    );
    assert!(observed);
    let queue = &owner
        .host_object(producer)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap()
        .production_queue;
    assert_eq!(
        queue.len(),
        1,
        "retirement must not remove the new identical entry"
    );
    assert_ne!(queue[0].runtime_entry_identity, entry_identity);
}

#[test]
fn mixed_unit_and_research_completion_receipts_follow_each_actual_producer_visit() {
    use crate::game_logic::buildings::{BuildingData, BuildingType};
    use crate::game_logic::host_production_log::HostProductionEvent;
    for unit_first in [true, false] {
        let (mut owner, _) = world(100, "PLAYER");
        owner
            .create_object_for_player(
                "ReceivingAccountingProducer",
                0,
                glam::Vec3::new(100.0, 0.0, 100.0),
            )
            .unwrap();
        let producers: Vec<_> = owner.objects.registered_keys().copied().collect();
        assert_eq!(producers.len(), 2);
        let mut unit = ThingTemplate::new("ReceivingOrderedUnit");
        unit.add_kind_of(crate::game_logic::KindOf::Vehicle)
            .set_health(100.0);
        owner.templates.insert(unit.name.clone(), unit.clone());
        for (index, id) in producers.iter().copied().enumerate() {
            let mut building = BuildingData::new(BuildingType::WarFactory);
            if (index == 0) == unit_first {
                assert!(building.add_to_queue("ReceivingOrderedUnit".into(), &unit));
                building.production_queue[0].total_time = 0.0;
            } else {
                assert!(owner.get_player_mut(0).unwrap().queue_upgrade(
                    "Upgrade_OwnedProductionAccounting",
                    &Resources {
                        supplies: 100,
                        power: 0
                    },
                    gamelogic::upgrade::UpgradeType::Player,
                    &mut Default::default()
                ));
                assert!(building.add_upgrade_to_queue(
                    "Upgrade_OwnedProductionAccounting".into(),
                    0.0,
                    Resources {
                        supplies: 100,
                        power: 0
                    }
                ));
            }
            owner.host_object_mut(id).unwrap().building_data = Some(building);
        }
        owner.production_events.clear();
        owner.update_production(1.0 / 30.0);
        owner.production_events.complete_boundary();
        let receipts = owner.production_events.snapshot_completed();
        assert_eq!(receipts.len(), 2);
        let names: Vec<_> = receipts
            .iter()
            .map(|event| match event {
                HostProductionEvent::Complete {
                    producer,
                    template_name,
                    ..
                } => (*producer, template_name.as_str()),
            })
            .collect();
        let expected = if unit_first {
            ["ReceivingOrderedUnit", "Upgrade_OwnedProductionAccounting"]
        } else {
            ["Upgrade_OwnedProductionAccounting", "ReceivingOrderedUnit"]
        };
        assert_eq!(
            names,
            [(producers[0], expected[0]), (producers[1], expected[1])]
        );
    }
}

#[test]
fn local_player_upgrade_dirties_receiving_ui_without_selected_cameo_but_object_does_not() {
    struct Ui(u32);
    impl crate::game_logic::ScriptUiEffects for Ui {
        fn weather_visible(&mut self, _: bool) {}
        fn letterbox(
            &mut self,
            _: &mut crate::presentation_frame::camera::SessionCamera,
            _: bool,
            _: bool,
            _upgrade_source: game_engine::common::thing::receiving_module_content::UpgradeTooltipSource<'_>,
            _game_ending: bool,
        ) {
        }
        fn mark_control_bar_dirty(&mut self) {
            self.0 += 1;
        }
    }
    for (kind, local, expected) in [
        ("PLAYER", true, 1),
        ("PLAYER", false, 0),
        ("OBJECT", true, 0),
    ] {
        let (mut owner, producer) = world(800, kind);
        owner.get_player_mut(0).unwrap().is_local = local;
        assert!(owner.get_player(0).unwrap().selected_objects.is_empty());
        let mut ui = Ui(0);
        let peer_ui = Ui(0);
        owner.host_apply_upgrade_production_completions_with_effects(
            vec![(
                Team::USA,
                "Upgrade_OwnedProductionAccounting".into(),
                producer,
            )],
            &mut crate::game_logic::ScriptPresentationEffects {
                ui: Some(&mut ui),
                ..Default::default()
            },
        );
        assert_eq!(ui.0, expected);
        assert_eq!(peer_ui.0, 0);
        assert_eq!(
            owner
                .get_player(0)
                .unwrap()
                .academy_stats
                .get_upgrades_purchased(),
            1
        );
        if kind == "PLAYER" {
            owner.add_completed_player_upgrade_with_effects(
                0,
                "Upgrade_OwnedProductionAccounting",
                &mut crate::game_logic::ScriptPresentationEffects {
                    ui: Some(&mut ui),
                    ..Default::default()
                },
            );
            assert_eq!(
                ui.0,
                expected * 2,
                "Player::addUpgrade dirties even repeated completion"
            );
        }
    }
}
