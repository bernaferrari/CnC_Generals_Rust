// C++ Common/RTS/Player.cpp:3132-3213,338-340,729-733,4056-4059.
// These are actual Player operations and full Snapshotable dispatch. No
// registry, synthetic module effect, ambient frame, or private counter write.

#[test]
fn radar_disable_proof_remains_available_while_disabled() {
    let mut player = Player::new(0);
    assert!(!player.has_radar());
    player.add_radar(false);
    player.disable_radar();
    assert!(!player.has_radar());
    player.add_radar(true);
    assert_eq!(player.get_radar_count(), 2);
    assert_eq!(player.get_disable_proof_radar_count(), 1);
    assert!(player.is_radar_disabled());
    assert!(player.has_radar(), "CPP proof count bypasses disabled flag");
    player.remove_radar(true);
    assert_eq!(player.get_radar_count(), 1);
    assert_eq!(player.get_disable_proof_radar_count(), 0);
    assert!(!player.has_radar());
    player.enable_radar();
    assert!(player.has_radar());
}

#[test]
fn radar_actual_player_operations_preserve_cpp_availability_edges() {
    // A transition is the candidate for CPP edge audio. This exercises actual
    // state decisions; it does not claim submission or the separate audio gates.
    let mut player = Player::new(0);
    let mut observed = Vec::new();
    let mut step = |mutate: fn(&mut Player)| {
        let before = player.has_radar();
        mutate(&mut player);
        observed.push((before, player.has_radar()));
    };
    step(|p| p.add_radar(false));
    step(|p| p.add_radar(false));
    step(Player::disable_radar);
    step(Player::disable_radar);
    step(|p| p.add_radar(true));
    step(Player::disable_radar);
    step(Player::enable_radar);
    step(|p| p.remove_radar(true));
    step(Player::disable_radar);
    step(Player::enable_radar);
    step(|p| p.remove_radar(false));
    step(|p| p.remove_radar(false));
    step(Player::disable_radar);
    step(Player::enable_radar);
    assert_eq!(
        observed,
        vec![
            (false, true),
            (true, true),
            (true, false),
            (false, false),
            (false, true),
            (true, true),
            (true, true),
            (true, true),
            (true, false),
            (false, true),
            (true, true),
            (true, false),
            (false, false),
            (false, false),
        ]
    );
    assert_eq!(player.get_radar_count(), 0);
    assert_eq!(player.get_disable_proof_radar_count(), 0);
    assert!(!player.is_radar_disabled());
}

#[test]
fn radar_same_index_players_keep_exact_instance_counts_and_visibility() {
    let mut first = Player::new(7);
    let mut second = Player::new(7);
    first.add_radar(true);
    first.add_radar(true);
    second.add_radar(false);
    first.disable_radar();
    second.disable_radar();
    assert!(first.has_radar());
    assert!(!second.has_radar());
    let untouched = Player::new(7);
    assert_eq!(untouched.get_radar_count(), 0);
    assert_eq!(first.get_radar_count(), 2);
    first.remove_radar(true);
    assert_eq!(first.get_radar_count(), 1);
    assert_eq!(first.get_disable_proof_radar_count(), 1);
    assert_eq!(second.get_radar_count(), 1);
    assert_eq!(second.get_disable_proof_radar_count(), 0);
    assert!(second.is_radar_disabled());
    drop(first);
    assert_eq!(second.get_radar_count(), 1);
    second.enable_radar();
    assert!(second.has_radar());
}

#[test]
fn radar_new_map_preserves_cpp_radar_state_and_sibling() {
    let mut first = Player::new(7);
    let mut second = Player::new(7);
    first.add_radar(true);
    first.disable_radar();
    second.add_radar(true);
    second.disable_radar();
    first.new_map();
    assert_eq!(first.get_radar_count(), 1, "CPP newMap only forwards to AI");
    assert_eq!(first.get_disable_proof_radar_count(), 1);
    assert!(first.is_radar_disabled() && first.has_radar());
    assert_eq!(second.get_radar_count(), 1);
    assert_eq!(second.get_disable_proof_radar_count(), 1);
    assert!(second.is_radar_disabled() && second.has_radar());
}

#[test]
fn radar_init_resets_exact_player_and_preserves_sibling() {
    let mut first = Player::new(7);
    let mut second = Player::new(7);
    first.add_radar(true);
    first.disable_radar();
    second.add_radar(true);
    second.disable_radar();
    first.init(Arc::new(PlayerTemplate::new(String::new())));
    assert_eq!(first.get_radar_count(), 0, "CPP init resets radar total");
    assert_eq!(first.get_disable_proof_radar_count(), 0);
    assert!(!first.is_radar_disabled());
    assert!(!first.has_radar());
    assert_eq!(second.get_radar_count(), 1);
    assert_eq!(second.get_disable_proof_radar_count(), 1);
    assert!(second.is_radar_disabled() && second.has_radar());
}

#[test]
fn radar_signed_proof_counter_is_not_clamped_and_round_trips_exactly() {
    let mut player = Player::new(3);
    player.add_radar(false);
    player.add_radar(false);
    // CPP asserts only total > 0. An inconsistent proof removal remains
    // signed, including through Xfer; hasRadar compares proof to zero.
    player.remove_radar(true);
    assert_eq!(player.get_radar_count(), 1);
    assert_eq!(player.get_disable_proof_radar_count(), -1);
    player.disable_radar();
    assert!(player.has_radar(), "CPP tests proof == 0, not proof > 0");
    let loaded = player_xfer_round_trip(player, 3);
    assert_eq!(loaded.get_radar_count(), 1);
    assert_eq!(loaded.get_disable_proof_radar_count(), -1);
    assert!(loaded.is_radar_disabled() && loaded.has_radar());
}

#[test]
fn radar_xfer_preserves_counts_disabled_and_dead_order_and_crc_surface() {
    let mut source = Player::new(3);
    let base_crc = player_crc(&source);
    source.restore_radar_state(0x0403_0201, 0x0807_0605, true);
    source.set_player_dead(true);
    assert_eq!(
        player_crc(&source),
        base_crc,
        "CPP CRC omits radar/dead save fields"
    );
    let mut bytes = Vec::new();
    {
        let mut xfer = XferSave::new(Cursor::new(&mut bytes), 1);
        Snapshotable::xfer(&mut source, &mut xfer).unwrap();
    }
    let mut expected = Vec::new();
    expected.extend_from_slice(&0x0403_0201i32.to_ne_bytes());
    expected.push(1); // C++ isPlayerDead between radar and proof counts.
    expected.extend_from_slice(&0x0807_0605i32.to_ne_bytes());
    expected.push(1); // C++ radarDisabled is one Bool byte.
    assert_eq!(
        bytes
            .windows(expected.len())
            .filter(|window| *window == expected.as_slice())
            .count(),
        1,
        "full Player Xfer contains exact C++ radar/dead/proof/disabled sequence"
    );
    let mut loaded = Player::new(3);
    let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
    Snapshotable::xfer(&mut loaded, &mut xfer).unwrap();
    assert_eq!(loaded.get_radar_count(), 0x0403_0201);
    assert_eq!(loaded.get_disable_proof_radar_count(), 0x0807_0605);
    assert!(loaded.is_radar_disabled());
    assert!(loaded.is_player_dead());
    assert!(loaded.has_radar());
    assert_eq!(player_crc(&loaded), base_crc);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "removeRadar: player radar count must be positive")]
fn radar_debug_remove_requires_positive_total_like_cpp_assert() {
    Player::new(0).remove_radar(false);
}

#[cfg(not(debug_assertions))]
#[test]
fn radar_release_remove_preserves_signed_cpp_counter_in_xfer() {
    let mut player = Player::new(0);
    player.remove_radar(false);
    assert_eq!(player.get_radar_count(), -1);
    assert!(!player.has_radar());
    let loaded = player_xfer_round_trip(player, 0);
    assert_eq!(loaded.get_radar_count(), -1);
    assert!(!loaded.has_radar());
}
