//! Actual kill-side ledger, bounty and nonpromoting XP ownership controls.
use super::*;
use crate::common::{AsciiString, GeometryInfo, Int, Real};
use crate::system::rank_info::{RankDefinition, RankDefinitionMode, RankInfoStore};

#[derive(Debug)]
struct AwardTemplate {
    inner: Arc<dyn ThingTemplate>,
    name: AsciiString,
    trainable: bool,
    skill: i32,
    value: i32,
}
impl ThingTemplate for AwardTemplate {
    fn get_name(&self) -> &AsciiString {
        &self.name
    }
    fn get_template_geometry_info(&self) -> GeometryInfo {
        self.inner.get_template_geometry_info()
    }
    fn calc_vision_range(&self) -> Real {
        self.inner.calc_vision_range()
    }
    fn calc_shroud_clearing_range(&self) -> Real {
        self.inner.calc_shroud_clearing_range()
    }
    fn is_kind_of(&self, kind: KindOf) -> bool {
        self.inner.is_kind_of(kind)
    }
    fn get_behavior_module_info(&self) -> &[TemplateModuleInfo] {
        self.inner.get_behavior_module_info()
    }
    fn get_max_health(&self) -> Real {
        self.inner.get_max_health()
    }
    fn get_build_cost(&self) -> Int {
        500
    }
    fn is_trainable(&self) -> bool {
        self.trainable
    }
    fn get_skill_point_value(&self, _level: usize) -> Int {
        self.skill
    }
    fn get_experience_value(&self, _level: usize) -> Int {
        self.value
    }
    fn get_experience_required(&self, level: usize) -> Int {
        [0, 100, 300, 600][level]
    }
}
fn ranks(second: i32) -> RankInfoStore {
    let mut ranks = RankInfoStore::new();
    for (rank, needed) in [(1, 0), (2, second), (3, 1000)] {
        ranks
            .apply_rank_definition(RankDefinition {
                rank,
                rank_name: format!("OwnedRank{rank}").into(),
                skill_points_needed: needed,
                science_purchase_points_granted: rank as i32,
                sciences_granted: Vec::new(),
                mode: RankDefinitionMode::Create,
            })
            .unwrap();
    }
    ranks
}
fn award_world(second_rank: i32) -> NativeMatch {
    award_world_with_trainability(second_rank, true, false)
}
fn award_world_with_trainability(
    second_rank: i32,
    source_trainable: bool,
    victim_trainable: bool,
) -> NativeMatch {
    let source = Arc::new(AwardTemplate {
        inner: ordinary_damage::damage_actor(false),
        name: "OwnedAwardSource".into(),
        trainable: source_trainable,
        skill: 0,
        value: 0,
    });
    let victim = Arc::new(AwardTemplate {
        inner: lethal_damage::instant_actor(),
        name: "OwnedAwardVictim".into(),
        trainable: victim_trainable,
        skill: 25,
        value: 20,
    });
    let mut game = lethal_damage::native_world_with_templates(source, victim);
    game.ranks = ranks(second_rank);
    let player = game.players.get_player(0).unwrap();
    let mut player = player.write().unwrap();
    player.initialize_owned_rank_ledger(&game.ranks, 0).unwrap();
    player.set_cash_bounty(0.1);
    drop(player);
    game
}
fn xp(owner: &Arc<RwLock<Object>>) -> i32 {
    owner
        .read()
        .unwrap()
        .with_experience_tracker(|t| t.get_current_experience())
        .unwrap()
}
#[test]
fn enemy_kill_awards_actual_receiving_rank_cash_and_xp_without_foreign_writes() {
    let mut game = award_world(20);
    let peer = award_world(200);
    assert_eq!(
        game.source.read().unwrap().get_id(),
        peer.source.read().unwrap().get_id()
    );
    let peer_player = peer.players.get_player(0).unwrap();
    let peer_guard = peer_player.write().unwrap();
    let target = Arc::clone(&game.target);
    game.operate(70, |world| {
        world
            .attempt_damage_owned(&target, &mut lethal_damage::fatal(1))
            .unwrap()
    });
    let player = game.players.get_player(0).unwrap();
    let player = player.read().unwrap();
    assert_eq!(player.get_rank_level(), 2);
    assert_eq!(player.get_skill_points(), 25);
    assert_eq!(player.get_science_purchase_points(), 3);
    assert_eq!(player.get_money().count_money(), 51);
    assert_eq!(player.get_score_keeper().get_total_money_earned(), 50);
    assert_eq!(xp(&game.source), 20);
    assert!(matches!(
        game.kill_awards.as_slice(),
        [
            super::super::ai_native_world::NativeKillAwardEvent::RankChanged {
                old_rank: 1,
                new_rank: 2,
                ..
            },
            super::super::ai_native_world::NativeKillAwardEvent::CashBounty { amount: 50, .. }
        ]
    ));
    assert_eq!(peer_guard.get_rank_level(), 1);
    assert_eq!(peer_guard.get_skill_points(), 0);
    assert_eq!(peer_guard.get_money().count_money(), 1);
    assert_eq!(xp(&peer.source), 0);
}
#[test]
fn under_construction_kill_counts_loss_but_does_not_award_rank_bounty_or_xp() {
    let mut game = award_world(20);
    let target = Arc::clone(&game.target);
    target.write().unwrap().set_status(
        crate::common::ObjectStatusMaskType::from_status(
            crate::common::ObjectStatusTypes::UnderConstruction,
        ),
        true,
    );
    game.operate(71, |world| {
        world
            .attempt_damage_owned(&target, &mut lethal_damage::fatal(1))
            .unwrap()
    });
    let player = game.players.get_player(0).unwrap();
    let player = player.read().unwrap();
    assert_eq!(player.get_skill_points(), 0);
    assert_eq!(player.get_money().count_money(), 1);
    assert_eq!(xp(&game.source), 0);
    assert!(game.kill_awards.is_empty());
}
#[test]
fn xp_promotion_is_rejected_before_victim_hp_or_player_awards_mutate() {
    let mut game = award_world(20);
    game.source
        .write()
        .unwrap()
        .with_experience_tracker_mut(|t| {
            t.add_experience_points_after_trainable_check(90, true, &[0, 100, 300, 600]);
        })
        .unwrap();
    let target = Arc::clone(&game.target);
    let before = target.read().unwrap().get_health();
    let error = game
        .operate(72, |world| {
            world.attempt_damage_owned(&target, &mut lethal_damage::fatal(1))
        })
        .unwrap_err();
    assert!(error.contains("promotion"), "{error}");
    assert_eq!(target.read().unwrap().get_health(), before);
    assert_eq!(xp(&game.source), 90);
    assert_eq!(
        game.players
            .get_player(0)
            .unwrap()
            .read()
            .unwrap()
            .get_skill_points(),
        0
    );
    assert!(game.kill_awards.is_empty());
}

#[test]
fn disappeared_experience_sink_falls_back_to_the_actual_source_tracker() {
    let mut game = award_world(20);
    game.source
        .write()
        .unwrap()
        .with_experience_tracker_mut(|t| t.set_experience_sink(999_991))
        .unwrap();
    let target = Arc::clone(&game.target);
    game.operate(73, |world| {
        world
            .attempt_damage_owned(&target, &mut lethal_damage::fatal(1))
            .unwrap()
    });
    assert_eq!(xp(&game.source), 20);
    assert_eq!(
        game.source
            .read()
            .unwrap()
            .with_experience_tracker(|t| t.get_experience_sink()),
        Some(999_991)
    );
}

#[test]
fn actual_tracker_xfer_preserves_scaled_next_kill_continuation() {
    use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;
    let mut original = award_world(20);
    original
        .source
        .write()
        .unwrap()
        .with_experience_tracker_mut(|t| {
            t.add_experience_points_after_trainable_check(10, true, &[0, 100, 300, 600]);
            t.set_experience_scalar(1.5);
        })
        .unwrap();
    let mut encoded = Cursor::new(Vec::new());
    original
        .source
        .write()
        .unwrap()
        .with_experience_tracker_mut(|t| t.xfer_state(&mut XferSave::new(&mut encoded, 1)))
        .unwrap()
        .unwrap();
    let mut restored = award_world(20);
    let mut input = Cursor::new(encoded.into_inner());
    restored
        .source
        .write()
        .unwrap()
        .with_experience_tracker_mut(|t| t.xfer_state(&mut XferLoad::new(&mut input, 1)))
        .unwrap()
        .unwrap();
    for game in [&mut original, &mut restored] {
        let target = Arc::clone(&game.target);
        game.operate(74, |world| {
            world
                .attempt_damage_owned(&target, &mut lethal_damage::fatal(1))
                .unwrap()
        });
    }
    assert_eq!(xp(&original.source), 40);
    assert_eq!(xp(&restored.source), 40);
    assert_eq!(
        original.source.read().unwrap().get_veterancy_level(),
        restored.source.read().unwrap().get_veterancy_level()
    );
    assert_eq!(original.kill_awards, restored.kill_awards);
}

#[test]
fn experience_sink_is_the_receiving_factory_object_before_its_death_callback() {
    let mut game = award_world_with_trainability(20, false, true);
    let peer = award_world_with_trainability(20, false, true);
    let target_id = game.target.read().unwrap().get_id();
    game.source
        .write()
        .unwrap()
        .with_experience_tracker_mut(|t| {
            t.set_experience_sink(target_id);
            t.set_experience_scalar(2.0);
        })
        .unwrap();
    let peer_target = Arc::clone(&peer.target);
    let peer_guard = peer_target.write().unwrap();
    let target = Arc::clone(&game.target);
    game.operate(75, |world| {
        world
            .attempt_damage_owned(&target, &mut lethal_damage::fatal(1))
            .unwrap()
    });
    assert_eq!(
        xp(&game.source),
        0,
        "source forwards XP even though it is not trainable"
    );
    assert_eq!(
        xp(&target),
        40,
        "sink receives source-scaled XP before installed onDie"
    );
    assert!(target.read().unwrap().is_destroyed());
    assert_eq!(
        peer_guard.with_experience_tracker(|t| t.get_current_experience()),
        Some(0)
    );
}

#[test]
fn ally_and_self_kills_do_not_award_enemy_rank_cash_or_experience() {
    for self_kill in [false, true] {
        let mut game = award_world(20);
        if !self_kill {
            game.players
                .get_player(0)
                .unwrap()
                .write()
                .unwrap()
                .set_player_relationship_by_index(1, crate::common::Relationship::Allies);
        }
        let target = Arc::clone(&game.target);
        let source = if self_kill {
            target.read().unwrap().get_id()
        } else {
            game.source.read().unwrap().get_id()
        };
        game.operate(76, |world| {
            world
                .attempt_damage_owned(&target, &mut lethal_damage::fatal(source))
                .unwrap()
        });
        assert_eq!(
            game.players
                .get_player(0)
                .unwrap()
                .read()
                .unwrap()
                .get_skill_points(),
            0
        );
        assert_eq!(
            game.players
                .get_player(0)
                .unwrap()
                .read()
                .unwrap()
                .get_money()
                .count_money(),
            1
        );
        assert_eq!(xp(&game.source), 0);
        assert!(game.kill_awards.is_empty());
    }
}
