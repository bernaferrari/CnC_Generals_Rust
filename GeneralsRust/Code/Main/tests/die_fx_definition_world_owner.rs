use generals_main::game_logic::{GameLogic, ObjectId, Player, Team};
use glam::Vec3;

const UNIT: &str = "SharedDieFxUnit";

fn world(fx: Option<&str>) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let behavior = fx
        .map(|name| format!("Behavior = FXListDie Death\n DeathFX = {name}\nEnd\n"))
        .unwrap_or_default();
    world
        .admit_object_ini_sources([(
            "DieFxOwner.ini".into(),
            format!(
                "Object {UNIT}\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n {behavior}End\n"
            ),
        )])
        .unwrap();
    world.add_player(Player::new(7, Team::USA, "Owner", true));
    let object = world.create_object_for_player(UNIT, 7, Vec3::ZERO).unwrap();
    (world, object)
}

#[test]
fn die_fx_definition_comes_from_the_receiving_objects_template() {
    let (mut first, first_id) = world(Some("FX_FirstWorld"));
    let (second, second_id) = world(Some("FX_SecondWorld"));
    let (without_fx, absent_id) = world(None);
    assert_eq!(first_id, second_id);
    assert_eq!(first_id, absent_id);

    let name = |world: &GameLogic, id| {
        world
            .host_object(id)
            .unwrap()
            .fx_list_die
            .as_ref()
            .and_then(|fx| fx.death_fx.clone())
    };
    assert_eq!(name(&first, first_id).as_deref(), Some("FX_FirstWorld"));
    assert_eq!(name(&second, second_id).as_deref(), Some("FX_SecondWorld"));
    assert_eq!(name(&without_fx, absent_id), None);

    first.reset();
    drop(first);
    assert_eq!(name(&second, second_id).as_deref(), Some("FX_SecondWorld"));
}
