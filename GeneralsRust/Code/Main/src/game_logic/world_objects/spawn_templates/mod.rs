//! Host objects `impl GameLogic` — `spawn_templates`.
//! templates, vision, spawn_faction_base. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

/// Return every exact source locomotor from one named Object INI set.  The
/// source parser preserves outer declaration order, so duplicate SET_NORMAL
/// rows deliberately remain ambiguous rather than silently becoming the row
/// a lossy attribute map happened to retain.
fn unambiguous_locomotors_for_set(
    definition: &crate::assets::ObjectDefinition,
    set_name: &str,
) -> Option<Vec<String>> {
    let mut matching = definition
        .locomotor_sets
        .iter()
        .filter(|row| row.set_name.eq_ignore_ascii_case(set_name));
    let row = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    (!row.locomotor_names.is_empty()).then(|| row.locomotor_names.clone())
}

fn locomotor_member_names_from_raw(raw: &str) -> Vec<String> {
    let mut parts = raw.split_whitespace();
    let Some(first) = parts.next() else {
        return Vec::new();
    };
    let skip_set = first.eq_ignore_ascii_case("SET_NORMAL")
        || first.eq_ignore_ascii_case("SET_NORMAL_UPGRADED")
        || first.eq_ignore_ascii_case("SET_PANIC")
        || first.eq_ignore_ascii_case("SET_TAXIING")
        || first.eq_ignore_ascii_case("SET_FREEFALL")
        || first.eq_ignore_ascii_case("SET_WANDER")
        || first.eq_ignore_ascii_case("SET_SUPERSONIC")
        || first.eq_ignore_ascii_case("SET_SLUGGISH");
    let mut names = Vec::new();
    if !skip_set && !first.is_empty() && !first.eq_ignore_ascii_case("none") {
        names.push(first.to_string());
    }
    for part in parts {
        if !part.is_empty() && !part.eq_ignore_ascii_case("none") {
            names.push(part.to_string());
        }
    }
    names
}

fn parse_auto_acquire_idle_bits_from_ini(text: &str) -> u32 {
    use gamelogic::object::update::ai_update_interface::{
        AUTO_ACQUIRE_IDLE, AUTO_ACQUIRE_IDLE_ATTACK_BUILDINGS, AUTO_ACQUIRE_IDLE_NO,
        AUTO_ACQUIRE_IDLE_NOT_WHILE_ATTACKING, AUTO_ACQUIRE_IDLE_STEALTHED,
    };
    let mut bits = 0u32;
    for tok in text.split(|c: char| c.is_whitespace() || matches!(c, '+' | '|' | ',')) {
        let t = tok.trim();
        if t.is_empty() || t == "=" {
            continue;
        }
        match t.to_ascii_uppercase().as_str() {
            "YES" => bits |= AUTO_ACQUIRE_IDLE,
            "STEALTHED" => bits |= AUTO_ACQUIRE_IDLE_STEALTHED,
            "NO" => bits |= AUTO_ACQUIRE_IDLE_NO,
            "NOTWHILEATTACKING" => bits |= AUTO_ACQUIRE_IDLE_NOT_WHILE_ATTACKING,
            "ATTACK_BUILDINGS" => bits |= AUTO_ACQUIRE_IDLE_ATTACK_BUILDINGS,
            _ => {}
        }
    }
    bits
}

fn leftover_object_definition_for_live(
    name: &str,
    reskin_from: &str,
    properties: &std::collections::HashMap<String, String>,
) -> crate::assets::ObjectDefinition {
    let mut definition = if let Some(manager_arc) = get_asset_manager() {
        manager_arc.lock().ok().and_then(|manager| {
            manager
                .resolve_object_definition(name, None)
                .cloned()
                .or_else(|| {
                    (!reskin_from.is_empty())
                        .then(|| {
                            manager
                                .resolve_object_definition(reskin_from, None)
                                .cloned()
                        })
                        .flatten()
                        .map(|mut parent| {
                            parent.name = name.to_string();
                            parent.parent_name = Some(reskin_from.to_string());
                            parent
                        })
                })
        })
    } else {
        None
    }
    .unwrap_or_else(|| crate::assets::ObjectDefinition::new(name.to_string()));
    definition.apply_create_override_properties(properties);
    definition
}

fn overlay_leftover_object_create_overrides_to_live(
    name: &str,
    reskin_from: &str,
    properties: &std::collections::HashMap<String, String>,
) {
    if let Some(manager_arc) = get_asset_manager() {
        if let Ok(mut manager) = manager_arc.try_lock() {
            manager.overlay_object_create_overrides(name, reskin_from, properties);
        }
    }
}

fn leftover_thing_template_for_prereq(
    name: &str,
) -> Option<std::sync::Arc<game_engine::common::thing::thing_template::ThingTemplate>> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    factory.find_template(name, false)
}

/// Leftover `parseHeightToSpeed` / `height_to_speed`: `sqrt(|2*g*h|)`.
/// A leftover factory parse that ran before GameData Gravity (g=-1) stores
/// `sqrt(80)` — treat that as unparsed and re-convert with leftover gravity.
fn leftover_g1_min_fall_speed() -> f32 {
    (2.0 * 40.0_f32).sqrt()
}

fn leftover_min_fall_is_gravity_aware(speed: f32) -> bool {
    speed.is_finite() && speed > 0.0 && (speed - leftover_g1_min_fall_speed()).abs() > 1e-2
}

fn leftover_physics_min_fall_speed_for_damage(name: &str) -> Option<f32> {
    let leftover = leftover_thing_template_for_prereq(name)?;
    for entry in leftover.get_behavior_module_info().iter() {
        if !entry.name.as_str().eq_ignore_ascii_case("PhysicsBehavior") {
            continue;
        }
        if let Some(data) = entry
            .data
            .downcast_ref::<gamelogic::object::behavior::PhysicsBehaviorModuleData>()
        {
            return Some(data.min_fall_speed_for_damage);
        }
    }
    None
}

/// Leftover factory `m_prereqInfo` wins; else leftover parse_prerequisites_block.
fn apply_production_prerequisites_from_definition(
    template: &mut crate::game_logic::ThingTemplate,
    definition: &crate::assets::ObjectDefinition,
) {
    if let Some(leftover) = leftover_thing_template_for_prereq(&template.name)
        .or_else(|| leftover_thing_template_for_prereq(&definition.name))
    {
        template.set_production_prerequisites(leftover.get_prereqs().to_vec());
        return;
    }
    if definition.prerequisite_lines.is_empty() {
        return;
    }
    let lines: Vec<String> = definition
        .prerequisite_lines
        .iter()
        .map(|(key, value)| format!("{key} = {value}"))
        .collect();
    template.parse_prerequisites_from_ini_lines(&lines);
}

fn apply_locomotor_set_names_from_definition(
    template: &mut crate::game_logic::ThingTemplate,
    unambiguous_normal: Option<&[String]>,
    raw_locomotor: Option<&str>,
) {
    if let Some(names) = unambiguous_normal {
        if !names.is_empty() {
            template.set_locomotor_set_names(names);
            return;
        }
    }
    if let Some(raw) = raw_locomotor {
        let names = locomotor_member_names_from_raw(raw);
        if !names.is_empty() {
            template.set_locomotor_set_names(&names);
            return;
        }
    }
    let fallback =
        crate::game_logic::locomotor_bootstrap::locomotor_set_names_for_unit(&template.name);
    if fallback.len() >= 2 {
        template.set_locomotor_set_names(&fallback);
    }
}

#[inline]
fn definition_has_rider_change_contain(definition: &crate::assets::ObjectDefinition) -> bool {
    definition
        .behavior_modules
        .iter()
        .any(|module| module.class_name.eq_ignore_ascii_case("RiderChangeContain"))
}

fn host_unlook_persist_frames() -> u32 {
    crate::game_logic::host_gamedata_lobby_residual::UNLOOK_PERSIST_DURATION_FRAMES_RESIDUAL.max(0)
        as u32
}

fn container_blocks_passenger_look(container: &Object) -> bool {
    let kind = container.thing.template.contain_module.kind;
    kind != crate::game_logic::ContainModuleKind::None && !container.is_garrison_contain()
}

fn restamp_host_partition_look(
    last: &mut std::collections::HashMap<ObjectId, (f32, f32, f32, f32, u32)>,
    live: &mut std::collections::HashSet<ObjectId>,
    shroud_mgr: &mut gamelogic::system::shroud_manager::ShroudManager,
    cell_ops: &mut Vec<(gamelogic::common::Coord3D, f32, u32, bool)>,
    id: ObjectId,
    center: gamelogic::common::Coord3D,
    range: f32,
    mask: u32,
    persist: u32,
    frame: u32,
) {
    live.insert(id);
    let next = (center.x, center.y, center.z, range, mask);
    if let Some(prev) = last.get(&id).copied() {
        let same = (prev.0 - next.0).abs() < 1e-4
            && (prev.1 - next.1).abs() < 1e-4
            && (prev.2 - next.2).abs() < 1e-4
            && (prev.3 - next.3).abs() < 1e-4
            && prev.4 == next.4;
        if same {
            return;
        }
        let old = gamelogic::common::Coord3D::new(prev.0, prev.1, prev.2);
        shroud_mgr.queue_undo_shroud_reveal(&old, prev.3, prev.4, persist, frame);
        cell_ops.push((old, prev.3, prev.4, false));
    }
    shroud_mgr.do_shroud_reveal(&center, range, mask);
    cell_ops.push((center, range, mask, true));
    last.insert(id, next);
}

fn unlook_stale_host_partition_looks(
    last: &mut std::collections::HashMap<ObjectId, (f32, f32, f32, f32, u32)>,
    live: &std::collections::HashSet<ObjectId>,
    shroud_mgr: &mut gamelogic::system::shroud_manager::ShroudManager,
    cell_ops: &mut Vec<(gamelogic::common::Coord3D, f32, u32, bool)>,
    persist: u32,
    frame: u32,
) {
    let stale: Vec<ObjectId> = last
        .keys()
        .copied()
        .filter(|id| !live.contains(id))
        .collect();
    for id in stale {
        if let Some(prev) = last.remove(&id) {
            let old = gamelogic::common::Coord3D::new(prev.0, prev.1, prev.2);
            shroud_mgr.queue_undo_shroud_reveal(&old, prev.3, prev.4, persist, frame);
            cell_ops.push((old, prev.3, prev.4, false));
        }
    }
}

/// C++ Object::unshroud/shroud: undoShroudCover is immediate (not queued).
fn restamp_host_partition_shroud(
    last: &mut std::collections::HashMap<ObjectId, (f32, f32, f32, f32, u32)>,
    live: &mut std::collections::HashSet<ObjectId>,
    shroud_mgr: &mut gamelogic::system::shroud_manager::ShroudManager,
    id: ObjectId,
    center: gamelogic::common::Coord3D,
    range: f32,
    mask: u32,
) {
    live.insert(id);
    let next = (center.x, center.y, center.z, range, mask);
    if let Some(prev) = last.get(&id).copied() {
        let same = (prev.0 - next.0).abs() < 1e-4
            && (prev.1 - next.1).abs() < 1e-4
            && (prev.2 - next.2).abs() < 1e-4
            && (prev.3 - next.3).abs() < 1e-4
            && prev.4 == next.4;
        if same {
            return;
        }
        let old = gamelogic::common::Coord3D::new(prev.0, prev.1, prev.2);
        shroud_mgr.undo_shroud_cover(&old, prev.3, prev.4);
    }
    shroud_mgr.do_shroud_cover(&center, range, mask);
    last.insert(id, next);
}

fn unshroud_stale_host_partition_covers(
    last: &mut std::collections::HashMap<ObjectId, (f32, f32, f32, f32, u32)>,
    live: &std::collections::HashSet<ObjectId>,
    shroud_mgr: &mut gamelogic::system::shroud_manager::ShroudManager,
) {
    let stale: Vec<ObjectId> = last
        .keys()
        .copied()
        .filter(|id| !live.contains(id))
        .collect();
    for id in stale {
        if let Some(prev) = last.remove(&id) {
            let old = gamelogic::common::Coord3D::new(prev.0, prev.1, prev.2);
            shroud_mgr.undo_shroud_cover(&old, prev.3, prev.4);
        }
    }
}

mod definition;
mod metadata;
mod seeding;
mod setup;
mod vision;

#[cfg(test)]
mod tests;
