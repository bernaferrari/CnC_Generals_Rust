use game_engine::common::ini::ini_fx_list::{FXList, FXNugget, get_fx_list_store_mut};
use generals_main::game_logic::{GameLogic, Player, Team};
use glam::Vec3;

#[test]
fn missing_session_death_fx_does_not_play_a_foreign_global_sound() {
    let mut foreign = FXList::new("FX_ForeignDieSession".into());
    foreign.add_nugget(FXNugget::Sound {
        name: "ForeignDieSound".into(),
    });
    get_fx_list_store_mut().add_fx_list(foreign);

    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Attacker", true));
    world.add_player(Player::new(1, Team::China, "Victim", false));
    world.admit_object_ini_sources([("FXDieOwner.ini".into(),
        "Object FXDieOwner\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = FXListDie DieFx\n DeathFX = FX_ForeignDieSession\n End\nEnd\n".into(),
    )]).unwrap();
    let victim = world
        .create_object_for_player("FXDieOwner", 1, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    let (hits, _) = world.apply_scud_area_at(Vec3::new(30.0, 0.0, 0.0), None, Team::USA, false);
    assert!(hits > 0, "the actual lethal hit must reach the victim");
    let object = world
        .host_object(victim)
        .expect("onDie retains the victim until cleanup");
    assert!(!object.is_alive());
    assert!(
        object
            .pending_transition_damage_fx
            .iter()
            .all(|event| event.audio_name.as_deref() != Some("ForeignDieSound"))
    );
    assert!(
        world
            .queued_audio_events
            .iter()
            .all(|event| event.event_type != "ForeignDieSound")
    );
}
