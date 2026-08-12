use super::super::*;
use crate::game_logic::game_logic::GameLogic;
use crate::game_logic::{GameMode, Player, Team};
use glam::Vec3;

#[test]
fn own_team_objects_are_fully_visible_in_presentation_fow() {
    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    logic.clear_all_players();
    let mut p0 = Player::new(0, Team::USA, "Human", true);
    p0.is_alive = true;
    logic.add_player(p0);
    let id = logic
        .create_object("USA_Dozer", Team::USA, Vec3::new(50.0, 0.0, 50.0))
        .expect("dozer");
    // Poison FOW: without residual, unexplored would hide.
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let ro = frame.objects.iter().find(|o| o.id == id).expect("ro");
    assert!(
        ro.fow_visibility.should_render(),
        "own-team dozer must be presentation-visible"
    );
    assert!(
        ro.fow_visibility.visibility_alpha >= 1.0,
        "own-team FOW must be FULLY_VISIBLE, not fogged/hidden"
    );
    assert!(ro.is_mobile, "USA_Dozer must be presentation-mobile");
    assert_eq!(ro.team, Team::USA);
    // count_mobile_friendlies also requires KindOf::Selectable; INI-loaded
    // dozers may omit that bit. FOW residual is the own-team snapshot itself.
    assert!(
        frame
            .objects
            .iter()
            .any(|o| o.id == id && o.team == Team::USA && o.is_mobile && o.fow_visibility.should_render()),
        "own-team mobile dozer must remain in the presentation object list"
    );
}
