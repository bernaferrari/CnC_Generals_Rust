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
        assert!(ro.is_mobile, "USA_Dozer must be presentation-mobile");
        assert_eq!(frame.count_mobile_friendlies(Team::USA), 1);
    }
