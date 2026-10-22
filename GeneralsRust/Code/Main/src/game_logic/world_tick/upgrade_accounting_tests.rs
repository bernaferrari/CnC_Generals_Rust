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
        .create_object("ReceivingAccountingProducer", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    world.host_object_mut(producer).unwrap().owner_player_id = Some(0);
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
        assert_eq!(owner.get_player(0).unwrap().statistics.resources_spent, 0);
        owner.host_apply_upgrade_production_completions(vec![(
            Team::USA,
            "Upgrade_OwnedProductionAccounting".into(),
            producer,
        )]);
        let p = owner.get_player(0).unwrap();
        assert_eq!(p.resources.supplies, 4200);
        assert_eq!(p.statistics.resources_spent, 800);
        assert_eq!(p.academy_stats.get_upgrades_purchased(), 1);
        assert_eq!(
            p.has_unlocked_upgrade("Upgrade_OwnedProductionAccounting"),
            kind == "PLAYER"
        );
        assert_eq!(owner.get_player(1).unwrap().statistics.resources_spent, 0);
        assert_eq!(
            owner
                .get_player(1)
                .unwrap()
                .academy_stats
                .get_upgrades_purchased(),
            0
        );
        assert_eq!(peer.get_player(0).unwrap().statistics.resources_spent, 0);
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
        restored.get_player(0).unwrap().statistics.resources_spent,
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
        restored.get_player(0).unwrap().statistics.resources_spent,
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
        source.get_player(0).unwrap().statistics.resources_spent,
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
