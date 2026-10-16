use super::*;

#[test]
fn receiving_snow_lifecycle_and_snapshots_do_not_mutate_peer() {
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        module_path!(),
        "receiving_snow_lifecycle_and_snapshots_do_not_mutate_peer",
    ) {
        return;
    }
    let mut owner = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    assert!(
        owner
            .snow_render_state()
            .manager
            .starting_heights()
            .is_empty()
    );
    assert!(
        peer.snow_render_state()
            .manager
            .starting_heights()
            .is_empty()
    );
    let definition = crate::snow::WeatherSetting {
        snow_enabled: true,
        snow_box_dimensions: 64.,
        snow_velocity: 4.,
        ..Default::default()
    };
    owner.snow.init_with_definition(&definition, &mut |_, _| 7);
    owner.snow_definition = definition.clone();
    owner.update_cpp_snow_and_anim2d(0.75);
    owner.snow.set_visible(false);
    let snapshot = owner.snow_render_state();
    peer.snow.init_with_definition(&definition, &mut |_, _| 19);
    peer.update_cpp_snow_and_anim2d(0.25);
    peer.snow.reset();
    assert_eq!(owner.snow.time(), 0.75);
    assert!(!owner.snow.is_visible());
    let changed = crate::snow::WeatherSetting {
        snow_velocity: 8.,
        ..definition
    };
    owner.admit_snow_definition(&changed);
    assert_eq!(owner.snow.time(), 0.75);
    assert!(!owner.snow.is_visible());
    assert_eq!(owner.snow_definition.snow_velocity, 8.);
    assert_eq!(peer.snow.time(), 0.25);
    assert!(
        peer.snow
            .starting_heights()
            .iter()
            .all(|height| *height == 19.)
    );
    owner.snow.reset();
    assert!(owner.snow.is_visible());
    assert_eq!(owner.snow.time(), 0.75);
    assert!(!snapshot.manager.is_visible());
    assert_eq!(snapshot.manager.time(), 0.75);
    assert!(
        snapshot
            .manager
            .starting_heights()
            .iter()
            .all(|height| *height == 7.)
    );
    assert_eq!(snapshot.definition.snow_velocity, 4.);
}

#[test]
fn frame_writer_revalidates_registration_after_peer_takes_live_slot() {
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        module_path!(),
        "frame_writer_revalidates_registration_after_peer_takes_live_slot",
    ) {
        return;
    }
    let mut owner = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    register_live_game_client(&mut owner);
    {
        let (mut frame, snow) = owner.split_frame_and_snow();
        frame.set_frame(11);
        snow.set_visible(false);
        assert_eq!(live_game_client_frame(), Some(11));
        register_live_game_client(&mut peer);
        peer.set_frame(22);
        frame.set_frame(33);
        assert_eq!(live_game_client_frame(), Some(22));
    }
    assert_eq!(owner.frame, 33);
    assert!(!owner.snow.is_visible());
    assert!(peer.snow.is_visible());
    clear_live_game_client(&mut peer);
}

#[test]
fn base_definition_admission_preserves_cached_map_snow_until_next_weather_block() {
    let mut owner = GameClient::new().unwrap();
    owner.snow_base_definition = crate::snow::WeatherSetting {
        snow_box_dimensions: 73.,
        snow_velocity: 7.,
        snow_amplitude: 11.,
        ..Default::default()
    };
    let original = owner.snow_base_definition.clone();
    owner.snow.init_with_definition(&original, &mut |_, _| 9);
    owner.snow.update(0.5);
    owner.snow.set_visible(false);
    let changed = crate::snow::WeatherSetting {
        snow_box_dimensions: 31.,
        snow_velocity: 3.,
        snow_amplitude: 29.,
        ..Default::default()
    };
    owner.admit_snow_definition(&changed);
    let mut cached = owner.snow_render_state().manager;
    cached.set_visible(true);
    let noise = cached.starting_heights().to_vec();
    let geometry = cached.flake_positions_y_up([10., 20., 30.]);
    owner.admit_base_snow_definition();
    let restored = owner.snow_render_state();
    assert_eq!(restored.definition.snow_box_dimensions, 73.);
    assert_eq!(restored.definition.snow_velocity, 7.);
    assert_eq!(restored.definition.snow_amplitude, 11.);
    assert_eq!(restored.manager.time(), 0.5);
    assert!(!restored.manager.is_visible());
    assert_eq!(restored.manager.starting_heights(), noise.as_slice());
    let mut visible = restored.manager.clone();
    visible.set_visible(true);
    assert_eq!(visible.flake_positions_y_up([10., 20., 30.]), geometry);
    assert_eq!(geometry.len(), 30 * 30, "cached 31-unit box remains active");
    owner.admit_snow_definition(&original);
    let mut refreshed = owner.snow_render_state().manager;
    refreshed.set_visible(true);
    assert_eq!(
        refreshed.flake_positions_y_up([10., 20., 30.]).len(),
        72 * 72
    );
    assert_eq!(refreshed.time(), 0.5);
    assert!(!owner.snow.is_visible());
}

#[test]
fn receiving_client_draw_routes_owned_snow_without_double_update() {
    let draw = include_str!("impl_draw.rs");
    let body = draw
        .split("pub fn draw_display")
        .nth(1)
        .unwrap()
        .split("fn install_load_screen")
        .next()
        .unwrap();
    assert!(body.contains("self.snow_render_state()"));
    assert!(body.contains("draw_with_snow(&snow)"));
    assert!(!body.contains("snow.update("));
    let display = include_str!("../../display/display.rs");
    assert!(display.contains("terrain.render_client_view(&view_glam, &proj_glam, snow)"));
}
