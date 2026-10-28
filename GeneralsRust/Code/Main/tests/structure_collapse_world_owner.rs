use generals_main::game_logic::{GameLogic, ObjectAudio, Player, SessionAudio, Team};
use glam::Vec3;

fn world(delay_ms: u32, initial_fx: &str) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "structure-collapse-owner.ini".to_string(),
            format!(
                "Object SharedCollapseProbe\n  KindOf = STRUCTURE IMMOBILE\n  Body = ActiveBody BodyTag\n    MaxHealth = 100\n    InitialHealth = 100\n  End\n  Behavior = StructureCollapseUpdate ModuleTag_Sink\n    MinBurstDelay = {delay_ms}\n    MaxBurstDelay = {delay_ms}\n    FXList = INITIAL {initial_fx}\n  End\nEnd\n"
            ),
        )])
        .expect("admit receiving Object INI into this world");
    world.add_player(Player::new(1, Team::USA, "Collapse Owner", false));
    let id = world
        .create_object_for_player("SharedCollapseProbe", 1, Vec3::ZERO)
        .expect("create authored structure");
    (world, id)
}

#[test]
fn same_id_worlds_begin_collapse_from_their_own_admitted_ini() {
    let (mut first, first_id) = world(2200, "FX_FirstCollapseInitial");
    let (mut second, second_id) = world(4400, "FX_SecondCollapseInitial");
    assert_eq!(first_id, second_id);

    let mut first_audio = SessionAudio::default();
    let mut second_audio = SessionAudio::default();
    assert!(
        first
            .host_object_mut(first_id)
            .unwrap()
            .begin_structure_collapse(0, &mut ObjectAudio::queue(&mut first_audio),)
    );
    assert!(
        second
            .host_object_mut(second_id)
            .unwrap()
            .begin_structure_collapse(0, &mut ObjectAudio::queue(&mut second_audio),)
    );

    let first_data = first
        .host_object(first_id)
        .unwrap()
        .structure_collapse_data
        .as_ref()
        .unwrap();
    let second_data = second
        .host_object(second_id)
        .unwrap()
        .structure_collapse_data
        .as_ref()
        .unwrap();
    assert_eq!(first_data.min_burst_delay, 66);
    assert_eq!(
        first_data.fx_initial.as_deref(),
        Some("FX_FirstCollapseInitial")
    );
    assert_eq!(second_data.min_burst_delay, 132);
    assert_eq!(
        second_data.fx_initial.as_deref(),
        Some("FX_SecondCollapseInitial")
    );

    first
        .host_object_mut(first_id)
        .unwrap()
        .tick_structure_collapse(1);
    second
        .host_object_mut(second_id)
        .unwrap()
        .tick_structure_collapse(1);
    assert_eq!(
        first
            .host_object(first_id)
            .unwrap()
            .structure_collapse_data
            .as_ref()
            .unwrap()
            .min_burst_delay,
        66
    );
    assert_eq!(
        second
            .host_object(second_id)
            .unwrap()
            .structure_collapse_data
            .as_ref()
            .unwrap()
            .min_burst_delay,
        132
    );
}
