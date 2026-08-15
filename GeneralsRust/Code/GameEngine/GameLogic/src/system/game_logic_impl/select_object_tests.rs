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
}
