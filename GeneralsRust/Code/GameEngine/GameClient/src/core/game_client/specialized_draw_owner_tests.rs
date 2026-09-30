mod specialized_draw_owner {
    use super::*;

    fn tank_entry(id: u32, position: [f32; 3]) -> PresentationDrawableSync {
        let mut entry = presentation_drawable_sync_for_test(
            id,
            1,
            "AmericaTankCrusader",
            true,
            false,
            position,
            0.0,
        );
        entry.draw_module_names = Arc::new(vec!["W3DTankDraw".to_string()]);
        entry
    }

    fn snapshot(client: &GameClient, id: u32) -> Option<PresentationSpecializedDrawSnapshot> {
        client.presentation_specialized_draw_snapshot(id).cloned()
    }

    #[test]
    fn same_ids_in_two_clients_keep_independent_pose_and_treads() {
        let id = 910_031;
        let mut first = GameClient::new().unwrap();
        let mut entry = tank_entry(id, [0.0; 3]);
        first.sync_presentation_drawables([entry.clone()]);
        entry.position = [10.0, 0.0, 0.0];
        first.sync_presentation_drawables([entry.clone()]);
        let expected = snapshot(&first, id).unwrap().tread_uv;
        assert!((expected - 0.95).abs() < 0.00001);

        let mut second = GameClient::new().unwrap();
        let other = tank_entry(id, [100.0, 0.0, 0.0]);
        second.sync_presentation_drawables([other.clone()]);
        assert_eq!(snapshot(&first, id).unwrap().tread_uv, expected);
        assert_eq!(snapshot(&second, id).unwrap().tread_uv, 0.0);
        // Neither lookup order nor projection onto different tread leaves may tick state.
        for _ in 0..8 {
            let right = snapshot(&second, id).unwrap();
            let left = snapshot(&first, id).unwrap();
            assert_eq!(right.tread_uv_for_mesh("A.TREADSL"), Some([0.0, 0.0]));
            assert_eq!(left.tread_uv_for_mesh("A.TREADSL"), Some([expected, 0.0]));
        }
        second.sync_presentation_drawables([other]);
        assert_eq!(snapshot(&first, id).unwrap().tread_uv, expected);
        first.sync_presentation_drawables([entry]);
        assert_eq!(snapshot(&first, id).unwrap().tread_uv, expected);
    }

    #[test]
    fn constructing_client_does_not_inherit_another_clients_snapshot() {
        let id = 910_032;
        let mut first = GameClient::new().unwrap();
        first.sync_presentation_drawables([tank_entry(id, [0.0; 3])]);
        assert!(snapshot(&first, id).is_some());
        let second = GameClient::new().unwrap();
        assert!(snapshot(&second, id).is_none());
        assert!(snapshot(&first, id).is_some());
    }

    #[test]
    fn destroy_and_recreate_same_id_has_no_predecessor_pose() {
        let id = 910_033;
        let mut client = GameClient::new().unwrap();
        client.sync_presentation_drawables([tank_entry(id, [0.0; 3])]);
        client.sync_presentation_drawables([tank_entry(id, [10.0, 0.0, 0.0])]);
        assert!((snapshot(&client, id).unwrap().tread_uv - 0.95).abs() < 0.00001);
        let first_binding = client
            .presentation_direct_drawable_state(1, id)
            .unwrap()
            .binding_key;
        client.sync_presentation_drawables(std::iter::empty::<PresentationDrawableSync>());
        assert!(snapshot(&client, id).is_none());
        client.sync_presentation_drawables([tank_entry(id, [100.0, 0.0, 0.0])]);
        let replacement = client
            .presentation_direct_drawable_state(1, id)
            .unwrap()
            .binding_key;
        assert_ne!(replacement, first_binding);
        assert_eq!(snapshot(&client, id).unwrap().tread_uv, 0.0);
    }

    #[test]
    fn attached_module_draw_does_not_overwrite_advanced_tread_state() {
        let id = 910_034;
        let mut client = GameClient::new().unwrap();
        client.sync_presentation_drawables([tank_entry(id, [0.0; 3])]);
        client.sync_presentation_drawables([tank_entry(id, [10.0, 0.0, 0.0])]);
        let expected = snapshot(&client, id).unwrap().tread_uv;
        let drawable_id = client
            .presentation_direct_drawable_state(1, id)
            .unwrap()
            .binding_key
            .drawable_id;
        let drawable = client.find_drawable_by_id_mut(drawable_id).unwrap();
        let basic = drawable
            .as_mut()
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .unwrap();
        for module in basic.get_draw_modules_mut() {
            module.do_draw(
                &Matrix4::identity(),
                &Matrix4::identity(),
                &Matrix4::identity(),
            );
        }
        assert_eq!(snapshot(&client, id).unwrap().tread_uv, expected);
    }
}

mod specialized_draw_projection {
    use super::*;

    #[test]
    fn non_ascii_mesh_leaf_is_rejected_without_utf8_slicing() {
        let snapshot = PresentationSpecializedDrawSnapshot {
            kind: PresentationSpecializedDrawKind::Tank,
            module_name: "W3DTankDraw".into(),
            object_id: 0,
            tread_uv: 0.25,
            wheel_angle: 0.0,
            laser_width: 0.5,
            debris_state: 0,
            debris_anim_time: 0.0,
            model_name: String::new(),
            science_hidden: false,
        };
        for name in ["A.TREADéS", "A.ééé", "A.道路TREADSL", "A.TREA💥"] {
            assert_eq!(snapshot.tread_uv_for_mesh(name), None);
        }
        assert_eq!(snapshot.tread_uv_for_mesh("A.treadsl"), Some([0.25, 0.0]));
        assert_eq!(snapshot.tread_uv_for_mesh("A.TREADSR"), Some([0.75, 0.0]));
    }

    #[test]
    fn persistent_fx_receives_preceding_pose_not_new_snapshot_pose() {
        let mut entry = presentation_drawable_sync_for_test(
            910_041,
            1,
            "AmericaTruck",
            true,
            false,
            [10.0, 0.0, 1.0],
            0.5,
        );
        let context = live_host_tick_from_sync(&entry, Some(([0.0; 3], 0.0)));
        assert_eq!(context.vel_mag_sq, 100.0);
        assert_eq!(context.physics.speed, 10.0);
        assert_eq!(context.physics.turning, 0.5);
        assert!(context.physics.is_motive);
        assert!(context.physics.airborne);
        entry.position = [10.0, 0.0, 0.0];
        let initial = live_host_tick_from_sync(&entry, None);
        assert_eq!(initial.physics.speed, 0.0);
        assert_eq!(initial.physics.turning, 0.0);
        assert!(!initial.physics.is_motive);
    }

    #[test]
    fn unchanged_module_names_move_through_sync_without_copying_strings() {
        let id = 910_042;
        let mut client = GameClient::new().unwrap();
        let mut entry = presentation_drawable_sync_for_test(
            id,
            1,
            "AmericaTankCrusader",
            true,
            false,
            [0.0; 3],
            0.0,
        );
        entry.draw_module_names = Arc::new(vec!["W3DTankDraw".into()]);
        client.sync_presentation_drawables([entry.clone()]);
        let old = client.presentation_specialized_draw_snapshot(id).unwrap();
        let module_name_pointer = old.module_name.as_ptr();
        let model_name_pointer = old.model_name.as_ptr();
        entry.position = [10.0, 0.0, 0.0];
        client.sync_presentation_drawables([entry]);
        let current = client.presentation_specialized_draw_snapshot(id).unwrap();
        assert_eq!(current.module_name.as_ptr(), module_name_pointer);
        assert_eq!(current.model_name.as_ptr(), model_name_pointer);
        assert!((current.tread_uv - 0.95).abs() < 0.00001);
    }
}

#[test]
fn specialized_draw_owner_snapshot_iterator_rejects_foreign_epoch() {
    let mut client = GameClient::new().unwrap();
    let mut entry = presentation_drawable_sync_for_test(
        910_050,
        2,
        "AmericaTankCrusader",
        true,
        false,
        [0.0; 3],
        0.0,
    );
    entry.draw_module_names = Arc::new(vec!["W3DTankDraw".into()]);
    client.sync_presentation_drawables([entry]);
    assert_eq!(client.presentation_specialized_draw_snapshots(1).count(), 0);
    let mut owned = client.presentation_specialized_draw_snapshots(2);
    let (id, snapshot) = owned.next().unwrap();
    assert_eq!(id, 910_050);
    assert_eq!(snapshot.object_id, id);
    assert!(owned.next().is_none());
    assert_eq!(client.presentation_specialized_draw_snapshots(0).count(), 0);
}
