#[cfg(test)]
mod select_object_tests {
    use super::*;
    use crate::ai::object_registry::{register_legacy_object, unregister_legacy_object};
    use std::sync::{Arc, Mutex, OnceLock, RwLock};

    const SELECT_OBJECT_TEST_ID: ObjectID = 9_013_880;

    fn select_object_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("select_object test lock poisoned")
    }

    #[test]
    fn select_object_puts_id_in_player_selection() {
        // Given: a registered selectable object and player 0
        let _lock = select_object_test_lock();
        let mut player = Player::new(0);
        player.init_from_dict_defaults();
        {
            let players = player_list();
            let mut players = players.write().expect("player list lock");
            players.clear();
            players.add_player(Arc::new(RwLock::new(player)));
        }

        let mut template = crate::common::DefaultThingTemplate::new("SelectObjectTest".to_string());
        template.add_kind_of(KindOf::Selectable);
        template.add_kind_of(KindOf::AlwaysSelectable);
        let mut object = Object::new_test_from_template(
            SELECT_OBJECT_TEST_ID,
            100.0,
            Arc::new(template),
        );
        object.set_selectable(true);
        let object = Arc::new(RwLock::new(object));
        register_legacy_object(&object);

        let mut logic = GameLogic::new();
        logic
            .register_object(Arc::clone(&object))
            .expect("register object");

        // When: GameLogic::select_object (C++ GameLogic.cpp:2595)
        let mask = PlayerMaskType::from_bits_truncate(1);
        logic.select_object(SELECT_OBJECT_TEST_ID, true, mask, false);

        // Then: the object id is in player 0's current selection
        let players = player_list();
        let players = players.read().expect("player list lock");
        let selected = players
            .get_player(0)
            .expect("player 0")
            .read()
            .expect("player lock")
            .get_current_selection_ids();
        assert!(
            selected.contains(&SELECT_OBJECT_TEST_ID),
            "select_object must place id in player selection, got {selected:?}"
        );

        drop(players);
        unregister_legacy_object(SELECT_OBJECT_TEST_ID);
        OBJECT_REGISTRY.unregister_object(SELECT_OBJECT_TEST_ID);
    }

    fn register_test_player(index: i32) {
        let mut player = Player::new(index);
        player.init_from_dict_defaults();
        let players = player_list();
        let mut players = players.write().expect("player list lock");
        players.add_player(Arc::new(RwLock::new(player)));
    }

    fn make_object(id: ObjectID, kinds: &[KindOf]) -> Arc<RwLock<Object>> {
        let mut template =
            crate::common::DefaultThingTemplate::new(format!("SelectObjectTest{id}"));
        for kind in kinds {
            template.add_kind_of(*kind);
        }
        let mut object = Object::new_test_from_template(id, 100.0, Arc::new(template));
        object.set_selectable(true);
        let object = Arc::new(RwLock::new(object));
        register_legacy_object(&object);
        object
    }

    fn player_selection(index: i32) -> Vec<ObjectID> {
        let players = player_list();
        let players = players.read().expect("player list lock");
        players
            .get_player(index)
            .expect("player")
            .read()
            .expect("player lock")
            .get_current_selection_ids()
    }

    fn cleanup_object(id: ObjectID) {
        unregister_legacy_object(id);
        OBJECT_REGISTRY.unregister_object(id);
    }

    #[test]
    fn select_object_loops_every_player_in_mask() {
        // C++ GameLogic.cpp:2608-2629 — getEachPlayerFromMask then
        // setCurrentlySelectedAIGroup for each bit.
        let _lock = select_object_test_lock();
        {
            let players = player_list();
            let mut players = players.write().expect("player list lock");
            players.clear();
        }
        register_test_player(0);
        register_test_player(1);

        const ID: ObjectID = 9_013_881;
        let object = make_object(ID, &[KindOf::Selectable, KindOf::AlwaysSelectable]);
        let mut logic = GameLogic::new();
        logic.register_object(Arc::clone(&object)).expect("register");

        let mask = PlayerMaskType::from_bits_truncate(0b11);
        logic.select_object(ID, true, mask, false);

        assert!(
            player_selection(0).contains(&ID),
            "player 0 mask bit must receive the object"
        );
        assert!(
            player_selection(1).contains(&ID),
            "player 1 mask bit must receive the object"
        );

        cleanup_object(ID);
    }

    #[test]
    fn select_object_rejects_structure_when_adding_to_selection() {
        // C++ GameLogic.cpp:2602-2606 — !isMassSelectable && !createNewSelection
        // returns. Object.cpp:3024: structures are not mass-selectable.
        let _lock = select_object_test_lock();
        {
            let players = player_list();
            let mut players = players.write().expect("player list lock");
            players.clear();
        }
        register_test_player(0);

        const UNIT: ObjectID = 9_013_882;
        const BUILDING: ObjectID = 9_013_883;
        let unit = make_object(UNIT, &[KindOf::Selectable, KindOf::AlwaysSelectable]);
        let building = make_object(BUILDING, &[KindOf::Selectable, KindOf::Structure]);
        let mut logic = GameLogic::new();
        logic.register_object(Arc::clone(&unit)).expect("register unit");
        logic
            .register_object(Arc::clone(&building))
            .expect("register building");

        let mask = PlayerMaskType::from_bits_truncate(1);
        logic.select_object(UNIT, true, mask, false);
        logic.select_object(BUILDING, false, mask, false);

        let selected = player_selection(0);
        assert!(selected.contains(&UNIT), "unit stays selected, got {selected:?}");
        assert!(
            !selected.contains(&BUILDING),
            "structure must not join an existing selection, got {selected:?}"
        );

        cleanup_object(UNIT);
        cleanup_object(BUILDING);
    }

    #[test]
    fn select_object_allows_structure_on_create_new_selection() {
        // C++ GameLogic.cpp:2602 — mass-selectable gate skipped when
        // createNewSelection is true.
        let _lock = select_object_test_lock();
        {
            let players = player_list();
            let mut players = players.write().expect("player list lock");
            players.clear();
        }
        register_test_player(0);

        const BUILDING: ObjectID = 9_013_884;
        let building = make_object(BUILDING, &[KindOf::Selectable, KindOf::Structure]);
        let mut logic = GameLogic::new();
        logic
            .register_object(Arc::clone(&building))
            .expect("register building");

        let mask = PlayerMaskType::from_bits_truncate(1);
        logic.select_object(BUILDING, true, mask, false);

        assert!(
            player_selection(0).contains(&BUILDING),
            "createNewSelection must select a structure"
        );

        cleanup_object(BUILDING);
    }

}
