use super::*;

#[test]
fn retaliation_reads_the_victims_controller_not_faction_or_local_view() {
    let mut world = GameLogic::new();
    let mut remote = Player::new(4, Team::USA, "RemoteHuman", true);
    remote.is_local = false;
    remote.logical_retaliation_mode_enabled = true;
    world.add_player(remote);
    let mut unrelated = Player::new(5, Team::USA, "UnrelatedHuman", true);
    unrelated.is_local = true;
    unrelated.logical_retaliation_mode_enabled = true;
    world.add_player(unrelated);
    world.add_player(Player::new(6, Team::GLA, "Enemy", false));
    world
        .players
        .get_mut(&4)
        .unwrap()
        .set_map_relationship(6, gamelogic::common::Relationship::Enemies);
    world
        .players
        .get_mut(&6)
        .unwrap()
        .set_map_relationship(4, gamelogic::common::Relationship::Enemies);
    let victim_id = ObjectId(0xCA01);
    let damager_id = ObjectId(0xCA02);
    let mut victim = Object::new(ThingTemplate::new("Victim"), victim_id, Team::USA);
    victim.owner_player_id = Some(4);
    let mut damager = Object::new(ThingTemplate::new("Damager"), damager_id, Team::GLA);
    damager.owner_player_id = Some(6);
    world.objects.insert(victim_id, victim);
    world.objects.insert(damager_id, damager);
    assert!(
        world.should_retaliate_against_aggressor(victim_id, damager_id),
        "remote human is a real human controller"
    );
    world.players.get_mut(&4).unwrap().is_human = false;
    assert!(
        !world.should_retaliate_against_aggressor(victim_id, damager_id),
        "a same-faction local human cannot replace the victim's AI controller"
    );
    world.objects.get_mut(&victim_id).unwrap().owner_player_id = None;
    assert!(
        !world.should_retaliate_against_aggressor(victim_id, damager_id),
        "an unadmitted owner cannot be discovered by faction"
    );
}
