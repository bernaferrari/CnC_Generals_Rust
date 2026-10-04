//! Exact live combat owners for source-honesty contracts, not behavioral proof.

#[test]
fn combat_chase_pathfinds_cpp_surface() {
    // The chase emission path was split out of the old game_logic monolith:
    // the OOR/LOS emission windows live in world_tick/combat/object_target.rs, the
    // attack-path entry in world_save/world_paths.rs, and the firing-position
    // search in pathfinding/system_attack.rs (C++ AIUpdate combat chase +
    // Pathfinder::findAttackPath + isAttackViewBlockedByObstacle).
    let combat_src = crate::game_logic::residuals::harness::rust_fn_body(
        include_str!("../../world_tick/combat/object_target.rs"),
        "update_object_target_combat",
    )
    .expect("actual object-target method");
    let paths_src = include_str!("../../world_save/world_paths.rs");
    let find_src = include_str!("../../pathfinding/system_attack.rs");
    assert!(
        paths_src.contains("fn assign_unit_attack_path")
            && find_src.contains("fn find_attack_firing_position"),
        "combat chase must use findAttackPath residual (assign_unit_attack_path)"
    );
    let i = combat_src
        .find("Ready weapons but out of range")
        .expect("OOR chase comment");
    let w = &combat_src[i..];
    assert!(
        w.contains("assign_unit_attack_path"),
        "OOR combat chase must call assign_unit_attack_path"
    );
    let j = combat_src
        .find("if self.out_of_weapon_range_object(attacker_id, target_id)")
        .expect("actual current-weapon range/LOS gate");
    // The marker is a call, not a declaration; delimit its first branch.
    let start = combat_src[j..].find('{').unwrap() + j;
    let mut depth = 1;
    let mut end = start + 1;
    for (offset, byte) in combat_src.as_bytes()[start + 1..].iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            end = start + 1 + offset;
            break;
        }
    }
    let w2 = &combat_src[start..=end];
    assert!(
        w2.contains("assign_unit_attack_path"),
        "LOS-blocked chase must call assign_unit_attack_path"
    );
}

#[test]
fn attack_view_blocked_cpp_surface() {
    // Inspect each actual owner separately: the combat current-slot gate,
    // its range/obstacle query, and the underlying pathfinder LOS operation.
    let body = crate::game_logic::residuals::harness::rust_fn_body(
        include_str!("../../world_tick/combat/object_target.rs"),
        "update_object_target_combat",
    )
    .expect("actual object-target method");
    assert!(body.contains("if let Some(slot) = selected_slot"));
    assert!(
        body.contains("out_of_weapon_range_object(attacker_id, target_id)"),
        "combat must gate fire on the current-weapon range/LOS query"
    );
    let range = crate::game_logic::residuals::harness::rust_fn_body(
        include_str!("../../world_tick/attack.rs"),
        "out_of_weapon_range_object",
    )
    .expect("actual current-weapon query");
    assert!(range.contains("attack_view_blocked(unit_id, Some(victim_id), to)"));
    let paths = crate::game_logic::residuals::harness::rust_fn_body(
        include_str!("../../world_save/world_paths.rs"),
        "attack_view_blocked",
    )
    .expect("actual LOS owner");
    assert!(paths.contains("is_attack_view_blocked"));
}
