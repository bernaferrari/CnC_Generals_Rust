//! Behavior tests extracted from the original hero/weapons/airfields suite.
use super::*;

#[test]
fn support_states_path_approach_cpp_surface() {
    // The facade monolith split: path_approach_with_state now lives in
    // world_save/world_paths.rs with callers across the live split modules.
    let src = concat!(
        include_str!("../../world_save/world_paths.rs"),
        include_str!("../../world_scripts/unit_commands.rs"),
        include_str!("../../world_scripts/rebuild_dozer.rs"),
        include_str!("../../world_scripts/saboteur_car_bomb.rs"),
        include_str!("../../world_objects/ai_authority.rs"),
        include_str!("../../world_tick/production.rs"),
        include_str!("../../world_tick/crates.rs"),
    );
    // Scan the live split members directly; production callers dominate the
    // count (per-file #[cfg(test)] tails contribute only the _for_test shim).
    let prod = src;
    assert!(prod.contains("fn path_approach_with_state"));
    assert!(
        prod.matches("path_approach_with_state").count() >= 10,
        "support states should route OOR approaches through path_approach_with_state"
    );
}

#[test]
fn host_attack_los_gates_fire_through_building() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    for (name, kinds) in [
        (
            "LosAtk",
            vec![
                KindOf::Infantry,
                KindOf::Attackable,
                KindOf::AttackNeedsLineOfSight,
            ],
        ),
        ("LosTgt", vec![KindOf::Infantry, KindOf::Attackable]),
        ("LosWall", vec![KindOf::Structure]),
    ] {
        if !logic.templates.contains_key(name) {
            let mut tmpl = ThingTemplate::new(name);
            tmpl.set_health(200.0);
            for k in kinds {
                tmpl.add_kind_of(k);
            }
            logic.templates.insert(name.into(), tmpl);
        }
    }
    let atk = logic
        .create_object("LosAtk", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("atk");
    let wall = logic
        .create_object("LosWall", Team::Neutral, glam::Vec3::new(40.0, 0.0, 0.0))
        .expect("wall");
    let tgt = logic
        .create_object("LosTgt", Team::GLA, glam::Vec3::new(80.0, 0.0, 0.0))
        .expect("tgt");
    // Block every cell on the Bresenham line between attacker and target.
    let from = glam::Vec3::new(0.0, 0.0, 0.0);
    let to = glam::Vec3::new(80.0, 0.0, 0.0);
    let start = logic.pathfinding_system.grid.world_to_grid(from);
    let goal = logic.pathfinding_system.grid.world_to_grid(to);
    let mut x0 = start.x;
    let mut y0 = start.y;
    let x1 = goal.x;
    let y1 = goal.y;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    // Skip start; block intermediate cells only.
    loop {
        let e2 = 2 * err;
        if e2 >= dy {
            if x0 == x1 {
                break;
            }
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            if y0 == y1 {
                break;
            }
            err += dx;
            y0 += sy;
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        logic.set_pathfinding_static_block(x0, y0, true);
    }
    assert!(
        logic.pathfinding_system.is_attack_view_blocked(from, to),
        "static wall must block attack view start={start:?} goal={goal:?}"
    );
    if let Some(o) = logic.objects.get_mut(&atk) {
        o.weapon = Some(Weapon {
            damage: 25.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ..Weapon::default()
        });
        o.target = Some(tgt);
        o.set_ai_state(AIState::Attacking);
        o.set_status_attacking(true);
    }
    let hp_before = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    logic.update_combat(&[atk, tgt, wall], 1.0 / 30.0);
    let hp_after = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        (hp_after - hp_before).abs() < 0.01,
        "LOS-blocked attacker must not damage target through static obstacle (hp {hp_before} -> {hp_after})"
    );
    let _ = wall;
}

#[test]
fn host_attack_los_allows_fire_in_open() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    for (name, kinds) in [
        (
            "LosAtk2",
            vec![
                KindOf::Infantry,
                KindOf::Attackable,
                KindOf::AttackNeedsLineOfSight,
            ],
        ),
        ("LosTgt2", vec![KindOf::Infantry, KindOf::Attackable]),
    ] {
        if !logic.templates.contains_key(name) {
            let mut tmpl = ThingTemplate::new(name);
            tmpl.set_health(200.0);
            for k in kinds {
                tmpl.add_kind_of(k);
            }
            logic.templates.insert(name.into(), tmpl);
        }
    }
    let atk = logic
        .create_object("LosAtk2", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("atk");
    let tgt = logic
        .create_object("LosTgt2", Team::GLA, glam::Vec3::new(30.0, 0.0, 0.0))
        .expect("tgt");
    if let Some(o) = logic.objects.get_mut(&atk) {
        o.weapon = Some(Weapon {
            damage: 25.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ..Weapon::default()
        });
        o.target = Some(tgt);
        o.set_ai_state(AIState::Attacking);
        o.set_status_attacking(true);
    }
    let hp_before = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    logic.update_combat(&[atk, tgt], 1.0 / 30.0);
    let hp_after = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hp_after < hp_before - 1.0,
        "open-field LOS must still allow fire (hp {hp_before} -> {hp_after})"
    );
}

#[test]
fn generic_object_fire_uses_weapon_ini_damage_type() {
    // C++ Weapon.cpp:1378-1380 dealDamage copies WeaponTemplate::m_damageType
    // onto DamageInfo; Armor.cpp:43-50 ArmorTemplate::adjustDamage then
    // bypasses only DAMAGE_UNRESISTABLE. Pre-fix live update_combat fallback
    // called take_damage_from → Unresistable and ignored Weapon.ini.
    use crate::game_logic::weapon_bootstrap::{PATHFINDER_SNIPER_WEAPON, ensure_host_weapon_store};
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    ensure_host_weapon_store();
    assert_eq!(
        crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(
            PATHFINDER_SNIPER_WEAPON
        ),
        crate::game_logic::combat::DamageType::Sniper
    );

    let mut logic = GameLogic::new();
    let mut atk_tpl = ThingTemplate::new("HqSzqaRifleman");
    atk_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name(PATHFINDER_SNIPER_WEAPON);
    logic.templates.insert("HqSzqaRifleman".into(), atk_tpl);

    let mut tank_tpl = ThingTemplate::new("HqSzqaTank");
    tank_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic.templates.insert("HqSzqaTank".into(), tank_tpl);

    let atk = logic
        .create_object("HqSzqaRifleman", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("atk");
    let tgt = logic
        .create_object("HqSzqaTank", Team::GLA, glam::Vec3::new(30.0, 0.0, 0.0))
        .expect("tgt");
    if let Some(o) = logic.objects.get_mut(&atk) {
        o.weapon = Some(Weapon {
            damage: 100.0,
            range: 300.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            projectile_speed: 999_999.0,
            ..Weapon::default()
        });
        o.target = Some(tgt);
        o.set_ai_state(AIState::Attacking);
        o.set_status_attacking(true);
    }
    let hp_before = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    logic.update_combat(&[atk, tgt], 1.0 / 30.0);
    let hp_after = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    // TankArmor SNIPER residual is 0% (Armor.ini). Unresistable would deal 100.
    assert!(
        (hp_after - hp_before).abs() < 0.01,
        "generic object-vs-object fire must use Weapon.ini SNIPER so TankArmor absorbs it (hp {hp_before} -> {hp_after})"
    );
}

#[test]
fn find_attack_path_picks_los_cell_not_target_footprint() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    for (name, kinds) in [
        (
            "FapAtk",
            vec![
                KindOf::Infantry,
                KindOf::Attackable,
                KindOf::AttackNeedsLineOfSight,
            ],
        ),
        ("FapTgt", vec![KindOf::Infantry, KindOf::Attackable]),
    ] {
        if !logic.templates.contains_key(name) {
            let mut tmpl = ThingTemplate::new(name);
            tmpl.set_health(200.0);
            for k in kinds {
                tmpl.add_kind_of(k);
            }
            logic.templates.insert(name.into(), tmpl);
        }
    }
    let atk = logic
        .create_object("FapAtk", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("atk");
    let tgt = logic
        .create_object("FapTgt", Team::GLA, glam::Vec3::new(80.0, 0.0, 0.0))
        .expect("tgt");
    // Wall between but leave a northern corridor open for flanking LOS.
    let from = glam::Vec3::new(0.0, 0.0, 0.0);
    let to = glam::Vec3::new(80.0, 0.0, 0.0);
    let start = logic.pathfinding_system.grid.world_to_grid(from);
    let goal = logic.pathfinding_system.grid.world_to_grid(to);
    let mut x0 = start.x;
    let mut y0 = start.y;
    let x1 = goal.x;
    let y1 = goal.y;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        let e2 = 2 * err;
        if e2 >= dy {
            if x0 == x1 {
                break;
            }
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            if y0 == y1 {
                break;
            }
            err += dx;
            y0 += sy;
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        // Block center line only (y==start.y); leave y+2 open for flank.
        logic.set_pathfinding_static_block(x0, y0, true);
    }
    if let Some(o) = logic.objects.get_mut(&atk) {
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ..Weapon::default()
        });
        o.target = Some(tgt);
        o.set_ai_state(AIState::Attacking);
    }
    assert!(
        logic.assign_unit_attack_path(atk, Some(tgt), to),
        "must find an attack path"
    );
    let unit = logic.objects.get(&atk).expect("atk after");
    let dest = unit
        .movement
        .path
        .last()
        .copied()
        .or(unit.movement.target_position)
        .expect("dest");
    // Final cell should not be the victim cell (findAttackPath goal is firing cell).
    let dest_cell = logic.pathfinding_system.grid.world_to_grid(dest);
    let victim_cell = logic.pathfinding_system.grid.world_to_grid(to);
    assert_ne!(
        dest_cell, victim_cell,
        "attack path should end on a firing cell, not victim footprint"
    );
    assert!(
        !logic.pathfinding_system.is_attack_view_blocked(dest, to),
        "firing cell must have clear LOS to victim"
    );
}

#[test]
fn structure_footprint_blocks_attack_los() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    for (name, kinds) in [
        (
            "SfAtk",
            vec![
                KindOf::Infantry,
                KindOf::Attackable,
                KindOf::AttackNeedsLineOfSight,
            ],
        ),
        ("SfTgt", vec![KindOf::Infantry, KindOf::Attackable]),
        ("SfWall", vec![KindOf::Structure]),
    ] {
        if !logic.templates.contains_key(name) {
            let mut tmpl = ThingTemplate::new(name);
            tmpl.set_health(500.0);
            for k in kinds {
                tmpl.add_kind_of(k);
            }
            logic.templates.insert(name.into(), tmpl);
        }
    }
    let atk = logic
        .create_object("SfAtk", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("atk");
    let wall = logic
        .create_object("SfWall", Team::Neutral, glam::Vec3::new(40.0, 0.0, 0.0))
        .expect("wall");
    if let Some(o) = logic.objects.get_mut(&wall) {
        o.selection_radius = 18.0;
    }
    // Re-block with larger footprint after radius bump.
    logic.sync_structure_path_blocks();
    let tgt = logic
        .create_object("SfTgt", Team::GLA, glam::Vec3::new(80.0, 0.0, 0.0))
        .expect("tgt");
    // Structure create must have static-blocked its footprint.
    assert!(
        logic.attack_view_blocked(atk, Some(tgt), glam::Vec3::new(80.0, 0.0, 0.0)),
        "structure between attacker and target must block attack LOS"
    );
    if let Some(o) = logic.objects.get_mut(&atk) {
        o.weapon = Some(Weapon {
            damage: 25.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ..Weapon::default()
        });
        o.target = Some(tgt);
        o.set_ai_state(AIState::Attacking);
        o.set_status_attacking(true);
    }
    let hp_before = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    logic.update_combat(&[atk, tgt, wall], 1.0 / 30.0);
    let hp_after = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        (hp_after - hp_before).abs() < 0.01,
        "must not fire through live structure footprint (hp {hp_before}->{hp_after})"
    );
}

#[test]
fn structure_path_block_cpp_surface() {
    // Facade split: structure path-blocking lives in world_subsystems.rs with
    // create/complete call sites in create_destroy_die.rs and production.rs.
    let src = concat!(
        include_str!("../../world_save/world_subsystems.rs"),
        include_str!("../../world_objects/create_destroy_die.rs"),
        include_str!("../../world_tick/production.rs"),
    );
    assert!(src.contains("fn sync_structure_path_blocks"));
    assert!(src.contains("fn block_structure_object_path"));
    assert!(src.contains("apply_structure_static_blocks"));
    assert!(
        src.contains("block_structure_object_path(id)")
            || src.contains("block_structure_object_path(completed_id)"),
        "create/complete must block structure footprints"
    );
}

#[test]
fn terrain_los_blocks_ridge_between_units() {
    let mut logic = GameLogic::new();
    // Install coarse height cache: flat 0 with a tall ridge at mid X cells.
    let w = logic.pathfinding_system.grid.width().max(8) as u32;
    let h = logic.pathfinding_system.grid.height().max(8) as u32;
    let mut heights = vec![0.0f32; (w * h) as usize];
    let mid = w / 2;
    for y in 0..h {
        for x in mid.saturating_sub(1)..=(mid + 1).min(w - 1) {
            heights[(y * w + x) as usize] = 80.0;
        }
    }
    assert!(
        logic.restore_terrain_heights_from_grid(w, h, &heights),
        "height cache install"
    );
    let from = glam::Vec3::new(0.0, 10.0, 0.0);
    let to = glam::Vec3::new(80.0, 10.0, 0.0);
    assert!(
        !logic.is_clear_line_of_sight_terrain(from, to),
        "ridge must block eye-line between low endpoints"
    );
    // Open sky above ridge still clear.
    let high_from = glam::Vec3::new(0.0, 100.0, 0.0);
    let high_to = glam::Vec3::new(80.0, 100.0, 0.0);
    assert!(
        logic.is_clear_line_of_sight_terrain(high_from, high_to),
        "high eye-line over ridge must stay clear"
    );
}

#[test]
fn attack_view_blocked_uses_terrain_los_surface() {
    // Facade split: attack_view_blocked + terrain LOS live in
    // world_save/world_paths.rs.
    let src = include_str!("../../world_save/world_paths.rs");
    assert!(src.contains("fn is_clear_line_of_sight_terrain"));
    assert!(src.contains("LOS_TERRAIN residual"));
    let i = src.find("pub fn attack_view_blocked").expect("avb");
    let w = &src[i..i + 2500.min(src.len() - i)];
    assert!(
        w.contains("is_clear_line_of_sight_terrain"),
        "attack_view_blocked must call terrain LOS"
    );
}
