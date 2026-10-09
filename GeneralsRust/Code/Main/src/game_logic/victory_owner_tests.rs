//! CPP VictoryConditions::cachePlayerPtrs (284-315) classifies the driving roster.
//! These exercise real Main admission and victory/presentation, not a census fixture.
use super::*;
use crate::game_logic::{GameLogic, ThingTemplate, VictoryType};
use crate::presentation_frame::PresentationFrame;
use glam::Vec3;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};

fn isolated(name: &str) -> bool {
    const MARKER: &str = "GENERALS_VICTORY_OBSERVATION_TEST";
    let module = module_path!().split_once("::").unwrap().1;
    let name = format!("{module}::{name}");
    if std::env::var(MARKER).as_deref() == Ok(name.as_str()) {
        return false;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([&name, "--exact", "--test-threads=1", "--nocapture"])
        .env(MARKER, &name)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let pipes: [Box<dyn Read + Send>; 2] = [
        Box::new(child.stdout.take().unwrap()),
        Box::new(child.stderr.take().unwrap()),
    ];
    let readers = pipes.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).unwrap();
            bytes
        })
    });
    let deadline = Instant::now() + Duration::from_secs(25);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            let _ = child.kill();
            break child.wait().unwrap();
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let output =
        readers.map(|reader| String::from_utf8_lossy(&reader.join().unwrap()).into_owned());
    assert!(!timed_out, "{name} exceeded deadline: {output:?}");
    assert!(status.success(), "{name}: {output:?}");
    assert!(
        output[0].contains("1 passed; 0 failed"),
        "exact child ran no regression: {output:?}"
    );
    true
}

struct ForeignRoster {
    previous: Option<gamelogic::player::PlayerList>,
}

impl ForeignRoster {
    fn install(observer: bool, civilian: bool) -> Self {
        let mut player = gamelogic::player::Player::new(0);
        if civilian {
            player.init(Arc::new(gamelogic::player::PlayerTemplate::new(
                "FactionCivilian".into(),
            )));
        }
        player.set_display_name("player0");
        player.set_observer(observer);
        let mut foreign = gamelogic::player::PlayerList::new();
        foreign.add_player(Arc::new(RwLock::new(player)));
        let previous = std::mem::replace(
            &mut *gamelogic::player::ThePlayerList().write().unwrap(),
            foreign,
        );
        Self {
            previous: Some(previous),
        }
    }
}

impl Drop for ForeignRoster {
    fn drop(&mut self) {
        *gamelogic::player::ThePlayerList()
            .write()
            .unwrap_or_else(|e| e.into_inner()) = self.previous.take().unwrap();
    }
}

fn admitted_match() -> (GameLogic, [ObjectId; 2]) {
    let mut world = GameLogic::new();
    world.game_mode = GameMode::Skirmish;
    world.frame = 30;
    for (id, team) in [(0, Team::USA), (1, Team::China)] {
        let mut player = Player::new(id, team, &format!("owner{id}"), id == 0);
        player.alliance_team = id as i32;
        player.resources.supplies = 1234 + id;
        world.add_player(player);
    }
    let mut template = ThingTemplate::new("VictoryRosterOwnedUnit");
    template
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    let ids = [
        world
            .create_object("VictoryRosterOwnedUnit", Team::USA, Vec3::ZERO)
            .unwrap(),
        world
            .create_object(
                "VictoryRosterOwnedUnit",
                Team::China,
                Vec3::new(20.0, 0.0, 0.0),
            )
            .unwrap(),
    ];
    for (index, id) in ids.iter().enumerate() {
        assert_eq!(
            world.host_object(*id).unwrap().owner_player_id,
            Some(index as u32)
        );
    }
    (world, ids)
}

fn destroy_armies(world: &mut GameLogic, ids: [ObjectId; 2]) {
    for id in ids {
        assert!(
            world
                .host_object_mut(id)
                .unwrap()
                .take_damage_from_immediate(1_000_000.0, None)
        );
        assert!(world.host_object(id).unwrap().status.destroyed);
    }
}

#[derive(Debug, PartialEq)]
struct GameplayObservation {
    objects: Vec<(ObjectId, f32, bool)>,
    players: Vec<(u32, bool, u32)>,
    removal_queue: Vec<ObjectId>,
    defeats: Vec<u32>,
    alliances: Vec<AllianceNotification>,
    rng: [u32; 6],
}

fn gameplay_observation(world: &GameLogic) -> GameplayObservation {
    let mut objects: Vec<_> = world
        .host_objects()
        .values()
        .map(|o| (o.id, o.health.current, o.status.destroyed))
        .collect();
    objects.sort_by_key(|o| o.0);
    let mut players: Vec<_> = world
        .players
        .values()
        .map(|p| (p.id, p.is_alive, p.resources.supplies))
        .collect();
    players.sort_by_key(|p| p.0);
    GameplayObservation {
        objects,
        players,
        removal_queue: world.objects_to_destroy.iter().map(|e| e.id).collect(),
        defeats: world.peek_defeat_events().to_vec(),
        alliances: world.peek_alliance_events().to_vec(),
        rng: world.logic_random.seed_words(),
    }
}

fn observation_match() -> (GameLogic, [ObjectId; 2]) {
    let (mut world, ids) = admitted_match();
    world
        .victory_conditions
        .set_victory_conditions(VictoryType::NO_BUILDINGS);
    let mut command_center = ThingTemplate::new("VictoryObservationCommandCenter");
    command_center
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::MpCountForVictory)
        .set_health(100.0);
    world
        .templates
        .insert(command_center.name.clone(), command_center);
    world
        .create_object("VictoryObservationCommandCenter", Team::USA, Vec3::ZERO)
        .unwrap();
    (world, ids)
}

fn assert_observers(
    world: &mut GameLogic,
    shadow: &crate::gameworld_shadow::GameWorldShadow,
    expected: Option<VictoryCondition>,
) {
    let before = gameplay_observation(world);
    for _ in 0..3 {
        let probe = shadow.probe(world);
        assert_eq!(probe.host_match_over, expected.is_some());
        assert_eq!(probe.victory_label, expected.map(|v| format!("{v:?}")));
        let probe = crate::authoritative_world::AuthorityProbe::capture_with_victory(world, 0);
        assert_eq!(probe.match_over, expected.is_some());
        assert_eq!(probe.victory_label, expected.map(|v| format!("{v:?}")));
        let frame = PresentationFrame::build_with_victory_for_engine(world, 0, Some(shadow));
        assert_eq!(frame.match_over, expected.is_some());
        assert_eq!(frame.victory_label, expected.map(|v| format!("{v:?}")));
        assert_eq!(
            gameplay_observation(world),
            before,
            "observation advanced gameplay"
        );
    }
}

fn observation_scenario(coupled: bool) {
    let (mut owner, ids) = observation_match();
    let (other, other_ids) = admitted_match();
    assert_eq!(ids, other_ids, "allocator-local IDs intentionally alias");
    let other_before = gameplay_observation(&other);
    let mut shadow = crate::gameworld_shadow::GameWorldShadow::new(64);
    shadow.sync_from_host(&owner);
    // China has a live unit but no victory-counting building. Observers must
    // not defeat it before C++'s Phase 15, even with an interleaved other owner.
    assert_observers(&mut owner, &shadow, None);
    assert!(owner.host_object(ids[1]).unwrap().is_alive());
    assert!(owner.peek_defeat_events().is_empty());
    assert_eq!(gameplay_observation(&other), other_before);

    let phase_frame = owner.get_frame();
    if coupled {
        let _tick = crate::gameworld_shadow::CoupledTickGuard::enter();
        crate::gameworld_shadow::with_coupled_shadow(&mut shadow, || owner.update());
        crate::gameworld_shadow::run_post_logic_shadow_boundary(Some(&mut shadow), &mut owner);
    } else {
        owner.update();
    }
    assert_eq!(
        owner.get_frame(),
        phase_frame + 1,
        "ordinary logic tick actually ran"
    );
    assert_eq!(
        owner.current_victory_condition(),
        Some(VictoryCondition::Winner(0))
    );
    assert!(!owner.host_object(ids[1]).unwrap().is_alive());
    assert_eq!(owner.get_player(1).unwrap().resources.supplies, 0);
    assert_eq!(owner.peek_defeat_events(), &[1]);
    assert_eq!(owner.victory_conditions.end_frame(), Some(phase_frame));
    assert_observers(&mut owner, &shadow, Some(VictoryCondition::Winner(0)));
    assert_eq!(owner.take_defeat_events(), vec![1]);
    let alliances = owner.take_alliance_events();
    assert!(!alliances.is_empty());
    assert_observers(&mut owner, &shadow, Some(VictoryCondition::Winner(0)));
    assert!(
        owner.peek_defeat_events().is_empty(),
        "observers must not replay drained defeat events"
    );
    assert!(
        owner.peek_alliance_events().is_empty(),
        "observers must not replay drained alliance events"
    );
    assert_eq!(gameplay_observation(&other), other_before);
    drop(other);
    assert_observers(&mut owner, &shadow, Some(VictoryCondition::Winner(0)));
    owner.reset();
    assert_observers(&mut owner, &shadow, None);
}

#[test]
fn probe_and_presentation_observe_victory_without_running_the_logic_phase() {
    if isolated("probe_and_presentation_observe_victory_without_running_the_logic_phase") {
        return;
    }
    observation_scenario(false);
}

#[test]
fn coupled_probe_and_presentation_observe_completed_victory() {
    if isolated("coupled_probe_and_presentation_observe_completed_victory") {
        return;
    }
    observation_scenario(true);
}

#[test]
fn completed_observation_tracks_each_evaluation_and_game_mode() {
    if isolated("completed_observation_tracks_each_evaluation_and_game_mode") {
        return;
    }
    let (mut world, _) = admitted_match();
    assert_eq!(world.evaluate_victory_condition(), None);
    world.players.get_mut(&1).unwrap().alliance_team = 0;
    assert_eq!(
        world.evaluate_victory_condition(),
        Some(VictoryCondition::Winner(0))
    );
    assert_eq!(
        world.current_victory_condition(),
        Some(VictoryCondition::Winner(0))
    );
    world.game_mode = GameMode::Shell;
    assert_eq!(world.current_victory_condition(), None);
    world.game_mode = GameMode::Skirmish;
    world.players.get_mut(&1).unwrap().alliance_team = 1;
    // Preserve the existing evaluator's exact None output even when end_frame
    // remains set. Inferring Draw from end_frame/winning_alliance is incorrect.
    assert_eq!(world.evaluate_victory_condition(), None);
    assert!(world.victory_conditions.end_frame().is_some());
    assert_eq!(world.current_victory_condition(), None);
    assert!(
        crate::authoritative_world::AuthorityProbe::capture_with_victory(&world, 0).match_over,
        "CPP singleAlliance/endFrame remains latched when the current winner query changes"
    );
    let mut shadow = crate::gameworld_shadow::GameWorldShadow::new(64);
    shadow.sync_from_host(&world);
    assert!(shadow.probe(&world).host_match_over);
    let before = gameplay_observation(&world);
    let frame = PresentationFrame::build_with_victory_for_engine(&mut world, 0, Some(&shadow));
    assert!(frame.match_over);
    assert!(
        frame.victory_label.is_none(),
        "completion must not manufacture a Draw result"
    );
    assert_eq!(gameplay_observation(&world), before);
    let ids: Vec<_> = world.host_objects().keys().copied().collect();
    for id in ids {
        world
            .host_object_mut(id)
            .unwrap()
            .take_damage_from_immediate(1_000_000.0, None);
    }
    assert_eq!(
        world.evaluate_victory_condition(),
        Some(VictoryCondition::Draw)
    );
    assert_eq!(
        world.current_victory_condition(),
        Some(VictoryCondition::Draw)
    );
}

#[test]
fn successful_restore_clears_only_transient_victory_observation() {
    if isolated("successful_restore_clears_only_transient_victory_observation") {
        return;
    }
    let (source, _) = admitted_match();
    let (mut receiver, _) = admitted_match();
    receiver.players.get_mut(&1).unwrap().alliance_team = 0;
    assert_eq!(
        receiver.evaluate_victory_condition(),
        Some(VictoryCondition::Winner(0))
    );
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    let before = gameplay_observation(&receiver);
    let saved_version = snapshot.version;
    snapshot.version = u32::MAX;
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut receiver)
            .is_err()
    );
    assert_eq!(gameplay_observation(&receiver), before);
    assert_eq!(
        receiver.current_victory_condition(),
        Some(VictoryCondition::Winner(0))
    );
    snapshot.version = saved_version;
    builder
        .restore_from_snapshot(&snapshot, &mut receiver)
        .unwrap();
    assert_eq!(receiver.current_victory_condition(), None);
    let mut shadow = crate::gameworld_shadow::GameWorldShadow::new(64);
    shadow.sync_from_host(&receiver);
    assert_observers(&mut receiver, &shadow, None);
    assert_eq!(source.current_victory_condition(), None);
    receiver.players.get_mut(&1).unwrap().alliance_team = 0;
    assert_eq!(
        receiver.evaluate_victory_condition(),
        Some(VictoryCondition::Winner(0))
    );
    snapshot.lifecycle_tail = vec![0xff];
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut receiver)
            .is_err()
    );
    assert_eq!(
        receiver.current_victory_condition(),
        None,
        "a partial direct restore cannot display the old world's result"
    );
    // This is projection invalidation, not proof that the preexisting victory
    // gameplay latches/defeat state survive save/load (tracked separately).
}

#[test]
fn foreign_observer_cannot_exempt_the_owners_dead_player_from_defeat() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let (mut owner, ids) = admitted_match();
    let (mut other, other_ids) = admitted_match();
    assert_eq!(ids, other_ids);
    other.get_player_mut(0).unwrap().is_observer = true;
    let _foreign = ForeignRoster::install(true, false);
    destroy_armies(&mut owner, ids);
    assert!(owner.evaluate_victory_condition().is_some());
    let frame = PresentationFrame::build_with_victory_for_engine(&mut owner, 0, None);
    assert!(frame.match_over);
    assert_eq!(
        owner.get_player(0).unwrap().resources.supplies,
        0,
        "the receiver's ordinary player must run killPlayer despite a foreign observer at the same index"
    );
    assert_eq!(owner.get_player(1).unwrap().resources.supplies, 0);
    assert_eq!(frame.local_supplies, 0);
    assert!(
        owner
            .victory_conditions
            .is_local_allied_defeat(&owner.players)
    );
    assert_eq!(other.get_player(0).unwrap().resources.supplies, 1234);
    assert!(other.host_object(other_ids[0]).unwrap().is_alive());
}

#[test]
fn foreign_civilian_cannot_exempt_the_owners_dead_player_from_defeat() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let (mut owner, ids) = admitted_match();
    let _foreign = ForeignRoster::install(false, true);
    destroy_armies(&mut owner, ids);
    assert!(owner.evaluate_victory_condition().is_some());
    let frame = PresentationFrame::build_with_victory_for_engine(&mut owner, 0, None);
    assert!(frame.match_over);
    assert_eq!(owner.get_player(0).unwrap().resources.supplies, 0);
    assert_eq!(frame.local_supplies, 0);
}

#[test]
fn foreign_observer_cannot_remove_a_living_owner_army_from_the_match() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let (mut owner, ids) = admitted_match();
    let _foreign = ForeignRoster::install(true, false);
    assert!(owner.evaluate_victory_condition().is_none());
    assert!(owner.host_object(ids[0]).unwrap().is_alive());
    assert_eq!(owner.get_player(0).unwrap().resources.supplies, 1234);
}

#[test]
fn owned_observer_remains_exempt_with_an_ordinary_foreign_slot() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let (mut owner, ids) = admitted_match();
    owner.get_player_mut(0).unwrap().is_observer = true;
    let _foreign = ForeignRoster::install(false, false);
    destroy_armies(&mut owner, ids);
    assert!(owner.evaluate_victory_condition().is_some());
    let frame = PresentationFrame::build_with_victory_for_engine(&mut owner, 0, None);
    assert!(frame.match_over);
    assert_eq!(owner.get_player(0).unwrap().resources.supplies, 1234);
    // CPP isLocalAlliedDefeat returns m_singleAllianceRemaining for observers.
    assert!(
        owner
            .victory_conditions
            .is_local_allied_defeat(&owner.players)
    );
    assert_eq!(owner.get_player(1).unwrap().resources.supplies, 0);
}

#[test]
fn unresolved_owned_template_remains_excluded_without_foreign_fallback() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let (mut owner, ids) = admitted_match();
    owner.player_template_bindings.insert(
        0,
        PlayerTemplateIdentity {
            template_name: "VictoryRosterMissingTemplate".into(),
            template_index: None,
        },
    );
    let _foreign = ForeignRoster::install(false, false);
    destroy_armies(&mut owner, ids);
    assert!(owner.evaluate_victory_condition().is_some());
    assert_eq!(owner.get_player(0).unwrap().resources.supplies, 1234);
    assert_eq!(owner.get_player(1).unwrap().resources.supplies, 0);
}
