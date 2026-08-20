//! Host supply-truck / warehouse / center helpers.
//!
//! C++: `SupplyTruckAIUpdate`, `SupplyWarehouseDockUpdate`, `SupplyCenterDockUpdate`.

use super::ObjectId;
use glam::Vec3;

/// C++ `PATHFIND_CELL_SIZE_F` (`AIPathfind.h:416`).
pub const PATHFIND_CELL_SIZE_F: f32 = 10.0;
/// C++ twitch range: `0.4 * PATHFIND_CELL_SIZE_F`.
pub const WAREHOUSE_TWITCH_RANGE: f32 = 0.4 * PATHFIND_CELL_SIZE_F;
/// C++ `REGROUP_SUCCESS_DISTANCE_SQUARED` (`SupplyTruckAIUpdate.cpp:42`).
pub const REGROUP_SUCCESS_DISTANCE_SQUARED: f32 = 225.0;
/// C++ `FindPositionOptions.maxRadius` for regroup (`SupplyTruckAIUpdate.cpp:588`).
pub const REGROUP_FIND_POSITION_RADIUS: f32 = 100.0;
/// C++ `GUI:AddCash` key (`SupplyCenterDockUpdate.cpp:129`).
pub const SUPPLY_CENTER_ADD_CASH_KEY: &str = "GUI:AddCash";
/// C++ `GameMakeColor(0,0,0,230)` OR'd onto player color.
pub const SUPPLY_CENTER_FLOATING_TEXT_ALPHA: u8 = 230;

/// C++ `SupplyTruckAIUpdate::getWarehouseScanDistance` — AI players get 2×.
pub fn warehouse_scan_distance(authored: f32, is_computer: bool) -> f32 {
    if authored <= 0.0 {
        return 0.0;
    }
    if is_computer {
        authored * 2.0
    } else {
        authored
    }
}

/// C++ `SupplyWarehouseDockUpdate::action` close-enough: 2D center vs `(r*2)²`.
pub fn warehouse_too_far_2d(
    docker_xz: (f32, f32),
    warehouse_xz: (f32, f32),
    docker_bounding_circle_radius: f32,
) -> bool {
    let close = docker_bounding_circle_radius * 2.0;
    let dx = docker_xz.0 - warehouse_xz.0;
    let dz = docker_xz.1 - warehouse_xz.1;
    dx * dx + dz * dz > close * close
}

/// Deterministic C++ `GameLogicRandomValue(-range, range)` twitch pair.
pub fn warehouse_twitch_delta(seed: u32, draw: u32) -> (f32, f32) {
    let x = crate::game_logic::host_rng_residual::pure_logic_random_real(
        seed,
        draw,
        -WAREHOUSE_TWITCH_RANGE,
        WAREHOUSE_TWITCH_RANGE,
    );
    let z = crate::game_logic::host_rng_residual::pure_logic_random_real(
        seed,
        draw.wrapping_add(1),
        -WAREHOUSE_TWITCH_RANGE,
        WAREHOUSE_TWITCH_RANGE,
    );
    (x, z)
}

/// C++ `gainOneBox` depleted-voice: play when no next warehouse, or next is
/// farther than `getWarehouseScanDistance()/4` (AI scan already 2×).
pub fn should_play_supplies_depleted_voice(
    next_warehouse_distance: Option<f32>,
    scan_distance: f32,
    voice: &str,
) -> bool {
    if voice.is_empty() {
        return false;
    }
    match next_warehouse_distance {
        None => true,
        Some(dist) => dist > scan_distance / 4.0,
    }
}

/// C++ `TheGameText->fetch("GUI:AddCash")` formatted with the deposit.
pub fn format_gui_add_cash(value: u32) -> String {
    format!("+${value}")
}

/// C++ hide popup only when the *center* is STEALTHED, not locally controlled,
/// and not DETECTED.
pub fn hide_stealth_supply_cash(
    center_stealthed: bool,
    center_locally_controlled: bool,
    center_detected: bool,
) -> bool {
    center_stealthed && !center_locally_controlled && !center_detected
}

/// C++ `setDockCrippled(true)` victim action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockCrippleVictimAction {
    None,
    KillGround,
    IdleAndForceWanting,
}

pub fn dock_cripple_victim_action(
    crippled: bool,
    docker_inside: bool,
    docker_airborne: bool,
) -> DockCrippleVictimAction {
    if !crippled {
        return DockCrippleVictimAction::None;
    }
    if docker_inside {
        if docker_airborne {
            DockCrippleVictimAction::None
        } else {
            DockCrippleVictimAction::KillGround
        }
    } else {
        DockCrippleVictimAction::IdleAndForceWanting
    }
}

/// C++ `UnitCrateCollide` `findPositionAround` maxRadius 20.
pub const UNIT_CRATE_FIND_POSITION_RADIUS: f32 = 20.0;

/// Ring search around `origin` (host Y-up) so spawned units do not stack.
pub fn find_position_around_xz(
    origin: Vec3,
    occupied: &[(Vec3, f32)],
    index: u32,
    seed: u32,
) -> Vec3 {
    let start_ang = (seed.wrapping_add(index.wrapping_mul(17)) as f32) * 0.37;
    let mut best = origin;
    for ring in 0..6u32 {
        let radius = (ring as f32) * (UNIT_CRATE_FIND_POSITION_RADIUS / 5.0);
        let steps = if ring == 0 { 1 } else { 8 };
        for step in 0..steps {
            let ang = start_ang + (step as f32) * (std::f32::consts::TAU / steps as f32);
            let candidate = Vec3::new(
                origin.x + ang.cos() * radius,
                origin.y,
                origin.z + ang.sin() * radius,
            );
            let blocked = occupied.iter().any(|(pos, r)| {
                let dx = pos.x - candidate.x;
                let dz = pos.z - candidate.z;
                dx * dx + dz * dz < (*r + 2.0) * (*r + 2.0)
            });
            if !blocked {
                return candidate;
            }
            best = candidate;
        }
    }
    best
}

/// C++ `Player::hasScience` residual — case-insensitive name match.
pub fn player_has_science<'a, I>(unlocked: I, required: &str) -> bool
where
    I: IntoIterator<Item = &'a str>,
{
    if required.is_empty() {
        return true;
    }
    let req = required.to_ascii_lowercase().replace('-', "_");
    unlocked.into_iter().any(|s| {
        let u = s.to_ascii_lowercase().replace('-', "_");
        u == req || u.ends_with(&req) || req.ends_with(&u)
    })
}

/// C++ KindOf mask test: every set bit in `mask` must be present on the killer.
pub fn killer_matches_kindof_mask(killer_kindof_names: &[&str], mask: u64) -> bool {
    if mask == 0 {
        return true;
    }
    let names = game_engine::common::system::kind_of::KIND_OF_BIT_NAMES;
    for (index, name) in names.iter().enumerate() {
        if index >= 64 {
            break;
        }
        if (mask & (1u64 << index)) == 0 {
            continue;
        }
        let want = name.to_ascii_uppercase();
        if !killer_kindof_names
            .iter()
            .any(|have| have.eq_ignore_ascii_case(&want))
        {
            return false;
        }
    }
    true
}

/// Host object id used as a twitch RNG seed.
pub fn twitch_seed(object: ObjectId, frame: u32) -> u32 {
    object.0.wrapping_mul(2654435761).wrapping_add(frame)
}

/// C++ `SabotageSupplyCenterCrateCollide`: `min(desired, victimMoney)`; 0 if broke.
pub fn steal_cash_clamped(available: u32, desired: u32) -> u32 {
    desired.min(available)
}

/// C++ `Drawable::updateDrawableSupplyStatus(startingBoxes, boxesStored)`.
/// Host cash is `boxes * ValuePerSupplyBox`; convert back for the crate visual.
pub fn drawable_supply_status_from_cash(
    starting_boxes: Option<u32>,
    stored_cash: u32,
) -> (u32, u32) {
    let value = crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX;
    let value = if value > 0 { value as u32 } else { 75 };
    let max_boxes = starting_boxes.unwrap_or(0);
    let current_boxes = stored_cash / value;
    (max_boxes, current_boxes)
}

/// Retail GLA SupplyCenterDock `GrantTemporaryStealth = 20000` ms → 600 frames.
/// America / China omit the field → module default 0.
pub fn grant_temporary_stealth_frames_for_center(template_name: &str) -> u32 {
    let n = template_name.to_ascii_lowercase();
    if n.contains("gla") && (n.contains("supply") || n.contains("stash")) {
        crate::game_logic::host_dock_contain_exit_heal_residual::SUPPLY_CENTER_DOCK_GLA_GRANT_STEALTH_FRAMES
    } else {
        crate::game_logic::host_dock_contain_exit_heal_residual::SUPPLY_CENTER_DOCK_DEFAULT_GRANT_STEALTH_FRAMES
    }
}

/// C++ `SupplyCenterDockUpdate::action` stealth grant gate:
/// center STEALTHED and (`isTemporaryGrant` or docker lacks `CAN_STEALTH`).
pub fn should_grant_temporary_stealth(
    center_stealthed: bool,
    grant_frames: u32,
    docker_is_temporary_grant: bool,
    docker_can_stealth: bool,
) -> bool {
    grant_frames > 0 && center_stealthed && (docker_is_temporary_grant || !docker_can_stealth)
}

/// C++ `DockUpdate::isClearToEnter`: only `m_activeDocker` may act.
/// Empty / dead active is reclaimable (update() promotes the next arriver).
pub fn dock_claim_active(
    current_active: Option<ObjectId>,
    current_active_alive: bool,
    claimant: ObjectId,
) -> Option<ObjectId> {
    match current_active {
        Some(id) if id == claimant => Some(id),
        Some(_) if current_active_alive => current_active,
        _ => Some(claimant),
    }
}

pub fn dock_is_clear_to_act(active: Option<ObjectId>, docker: ObjectId) -> bool {
    active == Some(docker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steal_from_broke_victim_is_zero() {
        assert_eq!(steal_cash_clamped(0, 1000), 0);
        assert_eq!(steal_cash_clamped(250, 1000), 250);
        assert_eq!(steal_cash_clamped(5000, 1000), 1000);
    }

    #[test]
    fn warehouse_drawable_boxes_follow_cash() {
        let (max, current) = drawable_supply_status_from_cash(Some(10), 10 * 75);
        assert_eq!(max, 10);
        assert_eq!(current, 10);
        let (_, after) = drawable_supply_status_from_cash(Some(10), 9 * 75);
        assert_eq!(after, 9);
        let (_, empty) = drawable_supply_status_from_cash(Some(10), 0);
        assert_eq!(empty, 0);
    }

    #[test]
    fn gla_stash_grants_temporary_stealth_america_does_not() {
        assert_eq!(
            grant_temporary_stealth_frames_for_center("GLASupplyStash"),
            600
        );
        assert_eq!(
            grant_temporary_stealth_frames_for_center("AmericaSupplyCenter"),
            0
        );
        assert!(should_grant_temporary_stealth(true, 600, false, false));
        assert!(!should_grant_temporary_stealth(true, 600, false, true));
        assert!(should_grant_temporary_stealth(true, 600, true, true));
        assert!(!should_grant_temporary_stealth(false, 600, false, false));
        assert!(!should_grant_temporary_stealth(true, 0, false, false));
    }

    #[test]
    fn single_active_docker_queue() {
        let a = ObjectId(1);
        let b = ObjectId(2);
        let first = dock_claim_active(None, false, a);
        assert_eq!(first, Some(a));
        assert!(dock_is_clear_to_act(first, a));
        assert!(!dock_is_clear_to_act(first, b));
        let blocked = dock_claim_active(Some(a), true, b);
        assert_eq!(blocked, Some(a));
        let after_dead = dock_claim_active(Some(a), false, b);
        assert_eq!(after_dead, Some(b));
    }
}

