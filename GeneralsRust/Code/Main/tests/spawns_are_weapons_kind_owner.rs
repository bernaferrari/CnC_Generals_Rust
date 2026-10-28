use generals_main::game_logic::{GameLogic, KindOf, Player, Team};
use generals_main::save_load::SnapshotBuilder;
use glam::Vec3;

const SITE: &str = "CustomStingerSiteLookalike";

fn world(authored_spawns_kind: bool) -> GameLogic {
    let mut world = GameLogic::new();
    let kinds = if authored_spawns_kind {
        "STRUCTURE SELECTABLE SPAWNS_ARE_THE_WEAPONS"
    } else {
        "STRUCTURE SELECTABLE"
    };
    world
        .admit_object_ini_sources([(
            "SpawnsAreWeapons.ini".into(),
            format!(
                "Object {SITE}\n KindOf = {kinds}\n Body = ActiveBody SiteBody\n MaxHealth = 300\n InitialHealth = 300\n End\nEnd\n"
            ),
        )])
        .expect("receiving Object INI is valid");
    world.add_player(Player::new(3, Team::USA, "Owner", true));
    world
}

#[test]
fn authored_spawns_kind_is_receiving_world_data_and_survives_snapshot_restore() {
    let mut authored = world(true);
    let mut lookalike = world(false);
    let authored_id = authored
        .create_object_for_player(SITE, 3, Vec3::new(25.0, 0.0, 35.0))
        .expect("authored site admitted");
    let lookalike_id = lookalike
        .create_object_for_player(SITE, 3, Vec3::new(25.0, 0.0, 35.0))
        .expect("lookalike admitted");
    assert_eq!(authored_id, lookalike_id);

    // This Main slice validates the authored KindOf bit only. Main does not
    // admit a general SpawnBehavior slave roster, so it cannot claim the
    // CPP orderSlavesDisabledUntil/clear timing path here.
    let authored_object = authored.host_object(authored_id).unwrap();
    assert!(authored_object.is_kind_of(KindOf::SpawnsAreTheWeapons));
    assert!(authored_object.is_spawns_are_the_weapons());
    let lookalike_object = lookalike.host_object(lookalike_id).unwrap();
    assert!(!lookalike_object.is_kind_of(KindOf::SpawnsAreTheWeapons));
    assert!(!lookalike_object.is_spawns_are_the_weapons());

    // Object snapshots persist the immutable template identity. Restore resolves
    // that name through the receiving world's admitted Object definitions.
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&authored)
        .expect("capture authored object");
    let mut restored = world(true);
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore against matching receiving content");
    assert!(
        restored
            .host_object(authored_id)
            .unwrap()
            .is_spawns_are_the_weapons()
    );

    // The same live lookalike remains unchanged while the authored world's
    // snapshot is captured/restored.
    assert!(
        !lookalike
            .host_object(lookalike_id)
            .unwrap()
            .is_spawns_are_the_weapons()
    );
}
