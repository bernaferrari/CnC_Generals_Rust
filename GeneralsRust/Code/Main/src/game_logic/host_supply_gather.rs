//! Host supply-truck / warehouse / center helpers.
//!
//! C++: `SupplyTruckAIUpdate`, `SupplyWarehouseDockUpdate`, `SupplyCenterDockUpdate`.

use crate::game_logic::{DockKind, ObjectId};
use glam::Vec3;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

fn live_dock_queues() -> &'static Mutex<HashMap<ObjectId, HostDockApproachQueue>> {
    static QUEUES: OnceLock<Mutex<HashMap<ObjectId, HostDockApproachQueue>>> = OnceLock::new();
    QUEUES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// C++ `DockUpdate::reserveApproachPosition` + `update` promote first arriver.
pub fn tick_live_dock_approach(
    dock_id: ObjectId,
    docker_id: ObjectId,
    number_approach_positions: i32,
    docker_alive: bool,
    current_active: Option<ObjectId>,
    current_active_alive: bool,
) -> DockApproachTick {
    if !docker_alive {
        return DockApproachTick::Blocked;
    }
    let Ok(mut map) = live_dock_queues().lock() else {
        return DockApproachTick::Blocked;
    };
    let queue = map
        .entry(dock_id)
        .or_insert_with(|| HostDockApproachQueue::new(number_approach_positions));
    queue.evict_dead(|_| true);
    if current_active == Some(docker_id) && current_active_alive {
        queue.on_enter_reached(docker_id);
        return DockApproachTick::ClearToAct;
    }
    if !queue.is_clear_to_approach(docker_id) {
        return DockApproachTick::Blocked;
    }
    let Some(_slot) = queue.reserve_approach_position(docker_id) else {
        return DockApproachTick::Blocked;
    };
    queue.on_approach_reached(docker_id);
    if queue.promote_active(current_active, current_active_alive, false) == Some(docker_id) {
        queue.on_enter_reached(docker_id);
        DockApproachTick::ClearToAct
    } else {
        DockApproachTick::Blocked
    }
}

pub fn cancel_live_dock_approach(dock_id: ObjectId, docker_id: ObjectId) {
    if let Ok(mut map) = live_dock_queues().lock() {
        if let Some(queue) = map.get_mut(&dock_id) {
            queue.cancel_docker(docker_id);
        }
    }
}

/// C++ `DYNAMIC_APPROACH_VECTOR_FLAG` (`DockUpdate.h:24`).
pub const DYNAMIC_APPROACH_VECTOR_FLAG: i32 = -1;
/// C++ `DEFAULT_APPROACH_VECTOR_SIZE` (`DockUpdate.h:19`).
pub const DEFAULT_APPROACH_VECTOR_SIZE: usize = 10;
/// Host arrival slop at a reserved approach point (`PATHFIND_CELL_SIZE_F`).
pub const DOCK_APPROACH_ARRIVAL_SLOP: f32 = PATHFIND_CELL_SIZE_F;

/// C++ `DockUpdate` approach-slot vectors (`m_approachPositionOwners` / `Reached`).
#[derive(Debug, Clone)]
pub struct HostDockApproachQueue {
    /// C++ `m_numberApproachPositions` (`-1` = dynamic / boneless infinite).
    pub number_approach_positions: i32,
    /// C++ `m_numberApproachPositionBones` (`0` = boneless bias).
    pub number_approach_position_bones: i32,
    /// Local `DockWaiting` bone positions, index 0 = first queue slot.
    pub waiting_bones: Vec<Vec3>,
    pub owners: Vec<Option<ObjectId>>,
    pub reached: Vec<bool>,
}

impl HostDockApproachQueue {
    pub fn new(number_approach_positions: i32) -> Self {
        let len = if number_approach_positions == DYNAMIC_APPROACH_VECTOR_FLAG {
            DEFAULT_APPROACH_VECTOR_SIZE
        } else {
            number_approach_positions.max(0) as usize
        };
        Self {
            number_approach_positions,
            number_approach_position_bones: 0,
            waiting_bones: Vec::new(),
            owners: vec![None; len],
            reached: vec![false; len],
        }
    }

    /// Install pristine `DockWaiting` bones. Empty keeps the boneless bias path.
    pub fn set_waiting_bones(&mut self, bones: Vec<Vec3>) {
        self.waiting_bones = bones;
        self.number_approach_position_bones = self.waiting_bones.len() as i32;
    }

    pub fn evict_dead(&mut self, mut is_alive: impl FnMut(ObjectId) -> bool) {
        for (owner, reached) in self.owners.iter_mut().zip(self.reached.iter_mut()) {
            if owner.is_some_and(|id| !is_alive(id)) {
                *owner = None;
                *reached = false;
            }
        }
    }

    /// C++ `DockUpdate::isClearToApproach`.
    pub fn is_clear_to_approach(&self, docker: ObjectId) -> bool {
        if self.number_approach_positions == DYNAMIC_APPROACH_VECTOR_FLAG {
            return true;
        }
        self.owners
            .iter()
            .any(|owner| owner.is_none() || *owner == Some(docker))
    }

    /// C++ `DockUpdate::reserveApproachPosition` — returns the reserved index.
    pub fn reserve_approach_position(&mut self, docker: ObjectId) -> Option<i32> {
        for (index, owner) in self.owners.iter().enumerate() {
            if *owner == Some(docker) {
                return Some(index as i32);
            }
            if owner.is_none() {
                self.owners[index] = Some(docker);
                self.reached[index] = false;
                return Some(index as i32);
            }
        }
        if self.number_approach_positions == DYNAMIC_APPROACH_VECTOR_FLAG {
            self.owners.push(Some(docker));
            self.reached.push(false);
            self.waiting_bones.push(Vec3::ZERO);
            return Some((self.owners.len() - 1) as i32);
        }
        None
    }

    /// C++ `DockUpdate::advanceApproachPosition`.
    pub fn advance_approach_position(&mut self, docker: ObjectId, index: i32) -> Option<i32> {
        if index <= 0 {
            return None;
        }
        let his = index as usize;
        if self.owners.get(his) != Some(&Some(docker)) {
            return None;
        }
        if self.owners.get(his - 1) != Some(&None) {
            return None;
        }
        self.owners[his - 1] = Some(docker);
        self.reached[his - 1] = false;
        self.owners[his] = None;
        self.reached[his] = false;
        Some((his - 1) as i32)
    }

    /// C++ `DockUpdate::isClearToAdvance`.
    pub fn is_clear_to_advance(&self, docker: ObjectId, index: i32) -> bool {
        if index <= 0 {
            return false;
        }
        let i = index as usize;
        let correct = self.owners.get(i) == Some(&Some(docker));
        let reached = self.reached.get(i).copied().unwrap_or(false);
        let next_free = self.owners.get(i - 1) == Some(&None);
        correct && reached && next_free
    }

    /// C++ `DockUpdate::onApproachReached`.
    pub fn on_approach_reached(&mut self, docker: ObjectId) {
        if let Some(index) = self.index_of(docker) {
            if let Some(reached) = self.reached.get_mut(index as usize) {
                *reached = true;
            }
        }
    }

    /// C++ `onEnterReached` — free the approach slot so the line can advance.
    pub fn on_enter_reached(&mut self, docker: ObjectId) {
        if let Some(index) = self.index_of(docker) {
            let i = index as usize;
            if let Some(owner) = self.owners.get_mut(i) {
                *owner = None;
            }
            if let Some(reached) = self.reached.get_mut(i) {
                *reached = false;
            }
        }
    }

    /// C++ `DockUpdate::cancelDock` slot half.
    pub fn cancel_docker(&mut self, docker: ObjectId) {
        for (owner, reached) in self.owners.iter_mut().zip(self.reached.iter_mut()) {
            if *owner == Some(docker) {
                *owner = None;
                *reached = false;
            }
        }
    }

    pub fn index_of(&self, docker: ObjectId) -> Option<i32> {
        self.owners
            .iter()
            .position(|owner| *owner == Some(docker))
            .map(|i| i as i32)
    }

    /// C++ `DockUpdate::update` — first arrived docker becomes `m_activeDocker`.
    pub fn promote_active(
        &self,
        current_active: Option<ObjectId>,
        current_active_alive: bool,
        crippled: bool,
    ) -> Option<ObjectId> {
        if current_active.is_some() && current_active_alive {
            return current_active;
        }
        if crippled {
            return None;
        }
        for (reached, owner) in self.reached.iter().zip(self.owners.iter()) {
            if *reached {
                return *owner;
            }
        }
        None
    }
}

/// Retail `NumberApproachPositions` for the live dock kinds the host owns.
/// Unknown / unauthored docks stay `0` (fail-closed: reserve refuses).
pub fn number_approach_positions_for_dock(
    template_name: &str,
    dock_kind: DockKind,
    is_repair_dock: bool,
    delete_when_empty: bool,
) -> i32 {
    let name = template_name.to_ascii_lowercase();
    if is_repair_dock {
        return super::host_repair::REPAIR_DOCK_NUMBER_APPROACH_POSITIONS as i32;
    }
    if name.contains("supplypile") {
        return 5;
    }
    if name.contains("supplydock") {
        return DYNAMIC_APPROACH_VECTOR_FLAG;
    }
    match dock_kind {
        DockKind::SupplyWarehouse => {
            if delete_when_empty {
                5
            } else {
                9
            }
        }
        DockKind::SupplyCenter => {
            if name.contains("china") || name.contains("gla") {
                DYNAMIC_APPROACH_VECTOR_FLAG
            } else {
                9
            }
        }
        DockKind::RailedTransport => 9,
        DockKind::None => 0,
    }
}

/// Result of one live `AIDock` tick against a host dock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DockApproachTick {
    /// Queue full, or waiting for the docker ahead. Do not act at the center.
    Blocked,
    /// Path to this reserved approach / enter point.
    PathTo(Vec3),
    /// C++ `isClearToEnter` — this docker is `m_activeDocker` and may act.
    ClearToAct,
}

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

/// C++ `SupplyTruckAIUpdate::gainOneBox` / `loseOneBox`
/// (`SupplyTruckAIUpdate.cpp:122-126`, `:164-168`):
/// `drawable->updateDrawableSupplyStatus(maxBoxes, m_numberBoxes)`.
/// Host collectors store cash; convert with retail `ValuePerSupplyBox`.
pub fn collector_drawable_supply_status(max_boxes: u32, stored_cash: u32) -> (u32, u32) {
    drawable_supply_status_from_cash(Some(max_boxes), stored_cash)
}

/// C++ `W3DModelDraw::updateDrawModuleSupplyStatus` (`W3DModelDraw.cpp:3907-3917`):
/// CARRYING when `currentSupply > 0`. Presentation stamps the mesh bit.
pub fn collector_carrying_from_boxes(current_boxes: u32) -> bool {
    current_boxes > 0
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

/// C++ `SupplyWarehouseDockUpdate::action` (`SupplyWarehouseDockUpdate.cpp:89-111`)
/// tentatively `--m_boxesStored`, then `SupplyTruckAIUpdate::gainOneBox`
/// (`SupplyTruckAIUpdate.cpp:134-135`). Already at MaxBoxes fails and the
/// warehouse `++m_boxesStored` takes the box back. Fail-closed: no transfer.
pub fn warehouse_action_transfer_one_box(
    warehouse_boxes: u32,
    collector_boxes: u32,
    max_boxes: u32,
) -> (u32, u32, bool) {
    if warehouse_boxes == 0 {
        return (0, collector_boxes, false);
    }
    if collector_boxes >= max_boxes {
        return (warehouse_boxes, collector_boxes, false);
    }
    (warehouse_boxes - 1, collector_boxes + 1, true)
}

/// Retail regular `AmericaVehicleChinook` `UpgradedSupplyBoost` (leftover INI parse = 4).
pub const REGULAR_CHINOOK_UPGRADED_SUPPLY_BOOST: u32 = 4;

/// C++ `ChinookAIUpdate` collectors: regular + Combat Chinook template names.
pub fn is_chinook_supply_collector(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    !n.is_empty() && n.contains("chinook")
}

/// C++ `ChinookAIUpdate::isAvailableForSupplying` (`ChinookAIUpdate.cpp:982-991`).
/// Non-Chinook collectors (trucks, workers) stay available.
pub fn chinook_available_for_supplying(
    is_chinook: bool,
    contain_count: usize,
    wanting_enter_or_exit: bool,
    is_overlord_style: bool,
) -> bool {
    if !is_chinook {
        return true;
    }
    !wanting_enter_or_exit && contain_count == 0 && !is_overlord_style
}

/// C++ `SupplyCenterDockUpdate::action` + `getUpgradedSupplyBoost`:
/// Chinooks return INI `UpgradedSupplyBoost` only with `Upgrade_AmericaSupplyLines`.
/// Trucks return 0. Worker shoes is a separate upgrade.
pub fn collector_supply_lines_boost(
    is_chinook: bool,
    is_combat_chinook: bool,
    authored_boost: u32,
    has_supply_lines: bool,
) -> u32 {
    if !is_chinook || !has_supply_lines {
        return 0;
    }
    if authored_boost > 0 {
        return authored_boost;
    }
    if is_combat_chinook {
        crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_UPGRADED_SUPPLY_BOOST
    } else {
        REGULAR_CHINOOK_UPGRADED_SUPPLY_BOOST
    }
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
    fn warehouse_action_does_not_debit_when_collector_already_at_max_boxes() {
        // Last remaining box + full collector: C++ take-back, warehouse stays.
        assert_eq!(
            warehouse_action_transfer_one_box(1, 4, 4),
            (1, 4, false)
        );
        assert_eq!(
            warehouse_action_transfer_one_box(10, 4, 4),
            (10, 4, false)
        );
        // Room for one more: debit and credit.
        assert_eq!(
            warehouse_action_transfer_one_box(1, 3, 4),
            (0, 4, true)
        );
        assert_eq!(
            warehouse_action_transfer_one_box(0, 2, 4),
            (0, 2, false)
        );
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
    fn collector_drawable_boxes_follow_cash_and_carrying() {
        // C++ gainOneBox / loseOneBox: updateDrawableSupplyStatus(maxBoxes, m_numberBoxes).
        assert_eq!(collector_drawable_supply_status(4, 0), (4, 0));
        assert_eq!(collector_drawable_supply_status(4, 75), (4, 1));
        assert_eq!(collector_drawable_supply_status(4, 4 * 75), (4, 4));
        assert!(!collector_carrying_from_boxes(0));
        assert!(collector_carrying_from_boxes(1));
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

    #[test]
    fn loaded_chinook_is_not_available_for_supplying() {
        // C++ ChinookAIUpdate::isAvailableForSupplying (ChinookAIUpdate.cpp:982-991).
        assert!(is_chinook_supply_collector("AmericaVehicleChinook"));
        assert!(is_chinook_supply_collector("AirF_AmericaVehicleChinook"));
        assert!(!is_chinook_supply_collector("ChinaVehicleSupplyTruck"));
        assert!(chinook_available_for_supplying(true, 0, false, false));
        assert!(!chinook_available_for_supplying(true, 1, false, false));
        assert!(!chinook_available_for_supplying(true, 0, true, false));
        assert!(!chinook_available_for_supplying(true, 0, false, true));
        assert!(chinook_available_for_supplying(false, 3, true, true));
    }

    #[test]
    fn supply_lines_boost_is_chinook_ini_only() {
        // C++ ChinookAIUpdate::getUpgradedSupplyBoost (ChinookAIUpdate.cpp:1644-1652)
        // vs SupplyTruckAIUpdate.h:196 returning 0.
        assert_eq!(collector_supply_lines_boost(false, false, 0, true), 0);
        assert_eq!(collector_supply_lines_boost(true, false, 0, false), 0);
        assert_eq!(
            collector_supply_lines_boost(true, false, 0, true),
            REGULAR_CHINOOK_UPGRADED_SUPPLY_BOOST
        );
        assert_eq!(collector_supply_lines_boost(true, true, 0, true), 60);
        assert_eq!(collector_supply_lines_boost(true, true, 4, true), 4);
    }
}

