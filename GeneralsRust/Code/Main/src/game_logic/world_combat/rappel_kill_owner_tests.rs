//! CPP AIRappelState scores before killing each occupant, then kills the
//! rappeller after two kills. All Body continuations belong to this receiver.
use super::*;

fn fixture() -> (GameLogic, ObjectId, ObjectId, Vec<ObjectId>) {
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "OwnedRappel.ini".to_owned(),
        r#"
Object OwnedRappelBuilding
  KindOf = STRUCTURE
  Body = ActiveBody BodyTag
    MaxHealth = 200
    InitialHealth = 200
  End
  Behavior = GarrisonContain ContainTag
    ContainMax = 3
    AllowInsideKindOf = INFANTRY
  End
End
Object OwnedRappeller
  KindOf = INFANTRY CAN_ATTACK
  IsTrainable = Yes
  Body = ActiveBody BodyTag
    MaxHealth = 100
    InitialHealth = 100
  End
  Behavior = SlowDeathBehavior DeathTag
    DestructionDelay = 5000
  End
End
Object OwnedRappelEnemy
  KindOf = INFANTRY CAN_ATTACK
  ExperienceValue = 10 10 10 10
  Body = ActiveBody BodyTag
    MaxHealth = 100
    InitialHealth = 100
  End
  Behavior = SlowDeathBehavior DeathTag
    DestructionDelay = 5000
  End
End
"#
        .to_owned(),
    )])
    .unwrap();
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(content).unwrap();
    world.frame = 42;
    let mut rappeller_player = Player::new(1, Team::USA, "Rappeller", true);
    rappeller_player.alliance_team = 1;
    let mut garrison_player = Player::new(2, Team::GLA, "Garrison", false);
    garrison_player.alliance_team = 2;
    world.add_player(rappeller_player);
    world.add_player(garrison_player);
    let building = world
        .create_object_for_player("OwnedRappelBuilding", 2, glam::Vec3::ZERO)
        .unwrap();
    let rappeller = world
        .create_object_for_player("OwnedRappeller", 1, glam::Vec3::ZERO)
        .unwrap();
    let mut enemies = Vec::new();
    for _ in 0..3 {
        let enemy = world
            .create_object_for_player("OwnedRappelEnemy", 2, glam::Vec3::ZERO)
            .unwrap();
        assert!(world.host_object_mut(building).unwrap().add_occupant(enemy));
        world.set_fixture_object_contained_by(enemy, Some(building));
        enemies.push(enemy);
    }
    (world, rappeller, building, enemies)
}

#[test]
fn two_ordered_combat_drop_kills_score_before_receiving_rappeller_death_and_save() {
    let (mut owner, rappeller, building, enemies) = fixture();
    let (peer, peer_rappeller, peer_building, peer_enemies) = fixture();
    assert_eq!(
        (rappeller, building, &enemies),
        (peer_rappeller, peer_building, &peer_enemies)
    );
    assert_eq!(
        owner.object_relationship(
            owner.host_object(rappeller).unwrap(),
            owner.host_object(enemies[0]).unwrap()
        ),
        gamelogic::common::Relationship::Enemies
    );
    owner.rappel_arrive_at_building(rappeller, building);
    let actor = owner.host_object(rappeller).unwrap();
    assert_eq!(actor.experience.current, 20.0);
    assert_eq!(actor.health.current, 0.0);
    assert_eq!(actor.last_damage_timestamp, Some(42));
    assert!(actor.status.on_die_started);
    for &enemy in &enemies[..2] {
        let victim = owner.host_object(enemy).unwrap();
        assert!(victim.kill_experience_awarded && victim.status.on_die_started);
        assert_eq!(victim.last_damage_timestamp, Some(42));
        assert_eq!(victim.contained_by, None);
    }
    assert_eq!(
        owner.host_object(building).unwrap().contained_units(),
        vec![enemies[2]]
    );
    assert_eq!(owner.host_object(enemies[2]).unwrap().health.current, 100.0);
    assert_eq!(
        peer.host_object(building).unwrap().contained_units(),
        peer_enemies
    );
    assert_eq!(peer.host_object(rappeller).unwrap().experience.current, 0.0);
    // Save the real delayed-death continuation at a settled boundary.
    // Immediate death correctly rejects capture until removal completes.
    assert!(!owner.has_pending_object_removals());
    for id in [rappeller, enemies[0], enemies[1]] {
        assert!(
            owner
                .host_object(id)
                .unwrap()
                .slow_death
                .as_ref()
                .unwrap()
                .is_active()
        );
    }
    let builder = crate::save_load::SnapshotBuilder::new();
    let encoded =
        bincode_legacy::serialize(&builder.create_world_snapshot(&owner).unwrap()).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap();
    let (mut restored, _, _, _) = fixture();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    for &id in &enemies[..2] {
        assert!(restored.host_object(id).unwrap().kill_experience_awarded);
    }
    restored.award_score_the_kill_experience(rappeller, enemies[0]);
    assert_eq!(
        restored.host_object(rappeller).unwrap().experience.current,
        20.0
    );
    assert_eq!(
        restored.host_object(building).unwrap().contained_units(),
        vec![enemies[2]]
    );
}

#[test]
fn combat_drop_uses_receiving_relationships_not_different_faction_names() {
    let (mut owner, rappeller, building, enemies) = fixture();
    owner.players.get_mut(&2).unwrap().alliance_team = 1;
    assert_eq!(
        owner.object_relationship(
            owner.host_object(rappeller).unwrap(),
            owner.host_object(enemies[0]).unwrap()
        ),
        gamelogic::common::Relationship::Allies
    );
    owner.rappel_arrive_at_building(rappeller, building);
    assert_eq!(owner.host_object(rappeller).unwrap().health.current, 100.0);
    assert_eq!(
        owner.host_object(rappeller).unwrap().experience.current,
        0.0
    );
    assert_eq!(
        owner.host_object(building).unwrap().contained_units(),
        enemies
    );
    for id in enemies {
        assert_eq!(owner.host_object(id).unwrap().health.current, 100.0);
        assert!(!owner.host_object(id).unwrap().kill_experience_awarded);
    }
}
