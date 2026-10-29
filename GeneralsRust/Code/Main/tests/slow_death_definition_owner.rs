use generals_main::game_logic::host_slow_death::HostSlowDeathIni;
use generals_main::game_logic::{GameLogic, ObjectAudio, ObjectId, Player, SessionAudio, Team};
use glam::Vec3;

const UNIT: &str = "SharedSlowDeathUnit";

fn world(delay_ms: Option<u32>, stale_delay_ms: Option<u32>) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let behavior = delay_ms
        .map(|delay| {
            format!("Behavior = SlowDeathBehavior Death\n DestructionDelay = {delay}\n End\n")
        })
        .unwrap_or_default();
    world
        .admit_object_ini_sources([(
            "SlowDeathOwner.ini".into(),
            format!(
                "Object {UNIT}\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n {behavior}End\n"
            ),
        )])
        .expect("receiving definition must be admitted");

    // Older template projections can be missing or stale after an override;
    // the complete admitted Object definition is the canonical content.
    world.templates.get_mut(UNIT).unwrap().authored_slow_death = stale_delay_ms.map(|delay| {
        vec![HostSlowDeathIni {
            destruction_delay_ms: delay,
            ..Default::default()
        }]
    });
    world.add_player(Player::new(7, Team::USA, "Owner", true));
    let object = world.create_object_for_player(UNIT, 7, Vec3::ZERO).unwrap();
    (world, object)
}

fn begin(world: &mut GameLogic, id: ObjectId) -> Option<u32> {
    let mut audio = SessionAudio::default();
    let object = world.host_object_mut(id).unwrap();
    object
        .begin_slow_death(10, &mut ObjectAudio::queue(&mut audio))
        .then(|| object.slow_death.as_ref().unwrap().destroy_at_frame)
}

#[test]
fn slow_death_uses_the_receiving_objects_definition() {
    let (mut first, first_id) = world(Some(1_000), Some(9_000));
    let (mut peer, peer_id) = world(Some(3_000), None);
    let (mut absent, absent_id) = world(None, Some(9_000));
    assert_eq!(first_id, peer_id);
    assert_eq!(first_id, absent_id);

    assert_eq!(begin(&mut first, first_id), Some(40));
    assert_eq!(begin(&mut absent, absent_id), None);
    first.reset();
    drop(first);
    assert_eq!(begin(&mut peer, peer_id), Some(100));
}
