//! Movement, waypoints, formation travel, patrol, and scatter.
#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
use crate::command_system::{
    CommandResult, CommandType, DropTarget, GameCommand, GuardTarget, PowerTarget,
    SpecialPowerType, WeaponSlot, WeaponTarget,
};
use crate::game_logic::game_logic::AudioEventRequest;
use crate::game_logic::{
    AIState, GameLogic, KindOf, ObjectId, ObjectType, PendingSpecialAbility, Resources, Team,
    radar_notifications::RadarKind,
};
use crate::localization;
use crate::ui::audio::translate_audio_event;
use gamelogic::common::AsciiString;
use gamelogic::common::types::Coord3D as LogicCoord3D;
use gamelogic::system::beacon_manager::get_beacon_manager;
use gamelogic::system::game_logic::current_frame;
use glam::Vec3;
use log::{debug, warn};
use std::collections::{HashMap, HashSet};

impl<'a> CommandExecutor<'a> {
    // === Movement Commands ===
    /// C++ `STD_WAYPOINT_CLAMP_MARGIN` (`AIGroup.cpp:1494`) = 4 * PATHFIND_CELL_SIZE_F.
    const STD_WAYPOINT_CLAMP_MARGIN: f32 = 4.0 * crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
    /// C++ `STD_AIRCRAFT_EXTRA_MARGIN` (`AIGroup.cpp:1495`) = 10 * PATHFIND_CELL_SIZE_F.
    const STD_AIRCRAFT_EXTRA_MARGIN: f32 = 10.0 * crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;

    /// C++ `GeometryInfo::getBoundingCircleRadius` from authored collision geom,
    /// not the pick/click `selection_radius` (`AIGroup.cpp:1790-1791`).
    fn bounding_circle_radius(unit: &crate::game_logic::object::Object) -> f32 {
        let g = &unit.thing.geometry;
        let half_x = ((g.bounds_max.x - g.bounds_min.x).abs() * 0.5).max(0.0);
        let half_z = ((g.bounds_max.z - g.bounds_min.z).abs() * 0.5).max(0.0);
        if half_x > 1e-3 && half_z > 1e-3 && (half_x - half_z).abs() > 1e-3 {
            (half_x * half_x + half_z * half_z).sqrt()
        } else {
            g.radius.max(half_x).max(half_z).max(1.0)
        }
    }

    /// C++ AIGroup tighten/scatter/move-bbox filter (`AIGroup.cpp:1625-1635`,
    /// `:1763`, `:1861`): DISABLED_HELD, KINDOF_IMMOBILE, or no AI. Stun /
    /// subdue / deploy must still count and still receive the order.
    fn group_ai_member_receives_move(unit: &crate::game_logic::object::Object) -> bool {
        if !unit.is_alive() {
            return false;
        }
        if unit.status.disabled_held || unit.contained_by.is_some() {
            return false;
        }
        if unit.is_kind_of(crate::game_logic::KindOf::Immobile)
            || unit.is_kind_of(crate::game_logic::KindOf::Structure)
        {
            return false;
        }
        unit.is_mobile() || unit.can_attack()
    }

    /// C++/leftover still queue the AI move when `can_move` is false (stun).
    fn queue_group_move_goal(&mut self, unit_id: ObjectId, dest: Vec3) -> bool {
        if self.game_logic.unit_command_move_to_moving(unit_id, dest) {
            return true;
        }
        let Some(unit) = self.game_logic.host_object_mut(unit_id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_destination(dest);
        unit.set_ai_state(AIState::Moving);
        true
    }

    /// C++ `theUnit->setFormationID(NO_FORMATION_ID)` in the free-move loop.
    fn dissolve_free_move_formation_stamps(&mut self, unit_ids: &[ObjectId]) {
        for &unit_id in unit_ids {
            let _ = self
                .game_logic
                .unit_command_set_formation(unit_id, 0, glam::Vec2::ZERO);
        }
    }

    /// C++ `pickAndPlayUnitVoiceResponse` force-move VoiceCrush
    /// (`CommandXlat.cpp:413-421`): dest drawable (`m_drawTarget`), not a
    /// pathfind-cell neighborhood. Gate is `canCrushOrSquish` (ALLIES + SquishCollide).
    fn dest_drawable_at(&self, dest: Vec3) -> Option<ObjectId> {
        let mut best: Option<(ObjectId, f32)> = None;
        for other in self.game_logic.objects.values() {
            if !other.is_alive() {
                continue;
            }
            let p = other.get_position();
            let dx = p.x - dest.x;
            let dz = p.z - dest.z;
            let dist_sq = dx * dx + dz * dz;
            let r = if other.thing.template.geometry_info.authored {
                other.thing.template.geometry_info.bounding_circle_radius()
            } else {
                other.selection_radius.max(1.0)
            };
            if dist_sq <= r * r && best.map(|(_, d)| dist_sq < d).unwrap_or(true) {
                best = Some((other.id, dist_sq));
            }
        }
        best.map(|(id, _)| id)
    }

    fn force_move_has_crush_target(&self, units: &[ObjectId], dest: Vec3) -> bool {
        use gamelogic::common::Relationship;
        let Some(tid) = self.dest_drawable_at(dest) else {
            return false;
        };
        let Some(target) = self.game_logic.host_object(tid) else {
            return false;
        };
        for &uid in units {
            if uid == tid {
                continue;
            }
            let Some(unit) = self.game_logic.host_object(uid) else {
                continue;
            };
            let ally = self.game_logic.object_relationship(unit, target) == Relationship::Allies;
            if unit.can_crush_or_squish(target, ally) {
                return true;
            }
        }
        false
    }

    /// C++ `KINDOF_PRODUCED_AT_HELIPAD` — host KindOf bank does not retain the
    /// bit, so reuse the existing helicopter template detector.
    fn is_produced_at_helipad(unit: &crate::game_logic::object::Object) -> bool {
        crate::game_logic::host_helicopter_slow_death::is_helicopter_slow_death_template(
            &unit.template_name,
        )
    }

    /// C++ `getHelicopterOffset` (`AIGroup.cpp:1799-1826`). Ground plane is XZ.
    fn helicopter_offset(pos: Vec3, idx: i32) -> Vec3 {
        if idx <= 0 {
            return pos;
        }
        const CIRCLE: f32 = 2.0 * std::f32::consts::PI;
        const HELI_DIAMETER: f32 = 70.0;
        let mut radius = HELI_DIAMETER;
        let mut circumference = radius * CIRCLE;
        let mut angle = 0.0f32;
        let mut angle_between = HELI_DIAMETER / circumference * CIRCLE;
        let mut h = 1;
        while h < idx {
            angle += angle_between;
            if angle > CIRCLE {
                radius += HELI_DIAMETER;
                circumference = radius * CIRCLE;
                angle_between = HELI_DIAMETER / circumference * CIRCLE;
                angle -= CIRCLE;
            }
            h += 1;
        }
        Vec3::new(
            pos.x + angle.sin() * radius,
            pos.y,
            pos.z + angle.cos() * radius,
        )
    }

    /// C++ `clampWaypointPosition` (`AIGroup.cpp:1497-1521`) after the
    /// helipad/aircraft extra-margin walk (`:1569-1593`).
    fn clamp_group_waypoint(&self, units: &[ObjectId], mut pos: Vec3) -> Vec3 {
        if !pos.x.is_finite() || !pos.z.is_finite() {
            return pos;
        }
        let mut extra_margin = 0.0f32;
        for &id in units {
            let Some(o) = self.game_logic.host_object(id) else {
                continue;
            };
            if Self::is_produced_at_helipad(o) {
                extra_margin = extra_margin.max(Self::bounding_circle_radius(o));
            } else if o.is_kind_of(crate::game_logic::KindOf::Aircraft) {
                extra_margin = extra_margin.max(Self::STD_AIRCRAFT_EXTRA_MARGIN);
            }
        }
        let margin = Self::STD_WAYPOINT_CLAMP_MARGIN + extra_margin;
        let (min, max) = self.game_logic.world_bounds();
        let lo_x = min.x + margin;
        let hi_x = max.x - margin;
        let lo_z = min.z + margin;
        let hi_z = max.z - margin;
        let inside = pos.x >= lo_x && pos.x <= hi_x && pos.z >= lo_z && pos.z <= hi_z;
        if !inside {
            let (cx_lo, cx_hi) = if lo_x <= hi_x {
                (lo_x, hi_x)
            } else {
                (hi_x, lo_x)
            };
            let (cz_lo, cz_hi) = if lo_z <= hi_z {
                (lo_z, hi_z)
            } else {
                (hi_z, lo_z)
            };
            pos.x = pos.x.clamp(cx_lo, cx_hi);
            pos.z = pos.z.clamp(cz_lo, cz_hi);
        }
        pos
    }

    pub(crate) fn execute_move(&mut self, units: &[ObjectId], destination: Vec3) -> CommandResult {
        // Wave 232: move last-writes via GameLogic unit_command_move_free.
        // C++ AIGroup::groupMoveToPosition (AIGroup.cpp:1559-1615): click-to-gather
        // only when !isFormation. A stamped formation always takes
        // friend_moveFormationToPos and is never tightened.
        // C++ clamps the click to map extent before tighten/formation (`:1592-1593`).
        let destination = self.clamp_group_waypoint(units, destination);
        let is_formation = self.group_is_stamped_formation(units);
        if !is_formation && self.should_tighten_group_move(units, destination) {
            let result = self.execute_tighten_to_position(units, destination);
            if matches!(result, CommandResult::Success) {
                self.play_context_move_voice(units);
            }
            return result;
        }
        if is_formation && units.len() > 1 {
            let result = self.execute_move_formation_to_position(units, destination);
            if matches!(result, CommandResult::Success) {
                self.play_context_move_voice(units);
            }
            return result;
        }
        self.apply_group_desired_speed(units, false);
        let goals = self.group_move_destinations(units, destination);
        // C++ free-move loop: setFormationID(NO_FORMATION_ID) (AIGroup.cpp:1681).
        let free_ids: Vec<ObjectId> = goals.iter().map(|(id, _)| *id).collect();
        self.dissolve_free_move_formation_stamps(&free_ids);
        if units.len() > 1 && self.compute_ground_path_should_group(units, destination) {
            if self
                .game_logic
                .assign_shared_group_paths(&goals, destination)
            {
                let moved: Vec<ObjectId> = goals.iter().map(|(id, _)| *id).collect();
                self.apply_player_stealth_mood_delay(&moved);
                self.play_context_move_voice(units);
                return CommandResult::Success;
            }
        }
        let mut moved: Vec<ObjectId> = Vec::new();
        for (unit_id, goal) in goals {
            if !self
                .game_logic
                .unit_command_move_free(unit_id, goal, destination)
            {
                if self.game_logic.host_object(unit_id).is_none() {
                    return CommandResult::InvalidTarget;
                }
                return CommandResult::InvalidCommand;
            }
            moved.push(unit_id);
            debug!("Unit {} moving to {:?}", unit_id.0, goal);
        }
        self.apply_player_stealth_mood_delay(&moved);
        self.play_context_move_voice(units);
        CommandResult::Success
    }

    pub(super) fn execute_move_to(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
        waypoints: &[Vec3],
    ) -> CommandResult {
        self.execute_move_to_with_voice(units, destination, waypoints, true)
    }

    /// C++ CommandXlat.cpp:1921-1937 MSG_DO_SALVAGE mimics MSG_DO_MOVETO,
    /// then pickAndPlay replaces VoiceMove with VoiceSalvage when valid
    /// (`CommandXlat.cpp:423-431`, skip=true).
    pub(super) fn execute_salvage(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
    ) -> CommandResult {
        let result = self.execute_move_to_with_voice(units, destination, &[], false);
        if matches!(result, CommandResult::Success) {
            self.play_salvage_or_move_voice(units);
        }
        result
    }

    fn execute_move_to_with_voice(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
        waypoints: &[Vec3],
        play_voice: bool,
    ) -> CommandResult {
        // Wave 232: move last-writes via GameLogic unit_command_move_to_waypoints.
        let destination = self.clamp_group_waypoint(units, destination);
        if waypoints.is_empty() && self.should_tighten_group_move(units, destination) {
            let result = self.execute_tighten_to_position(units, destination);
            if play_voice && matches!(result, CommandResult::Success) {
                self.play_context_move_voice(units);
            }
            return result;
        }
        self.apply_group_desired_speed(units, self.group_is_stamped_formation(units));
        let goals = self.group_move_destinations(units, destination);
        if !self.group_is_stamped_formation(units) {
            let free_ids: Vec<ObjectId> = goals.iter().map(|(id, _)| *id).collect();
            self.dissolve_free_move_formation_stamps(&free_ids);
        }
        let mut moved: Vec<ObjectId> = Vec::new();
        for (unit_id, goal) in goals {
            let goal = self
                .game_logic
                .adjust_group_member_goal(unit_id, goal, destination);
            if !self
                .game_logic
                .unit_command_move_to_waypoints(unit_id, goal, waypoints)
            {
                return CommandResult::InvalidCommand;
            }
            moved.push(unit_id);
            debug!("Unit {} moving via waypoints to {:?}", unit_id.0, goal);
        }
        self.apply_player_stealth_mood_delay(&moved);
        if play_voice {
            self.play_context_move_voice(units);
        }
        CommandResult::Success
    }

    /// C++ pickAndPlay VoiceMove / VoiceMoveUpgraded (`CommandXlat.cpp:384-443`).
    pub(super) fn play_context_move_voice(&mut self, units: &[ObjectId]) {
        self.game_logic.queue_picked_move_voice(units);
    }

    /// C++ `CommandXlat.cpp:423-443`: VoiceSalvage then VoiceMoveUpgraded overwrite.
    fn play_salvage_or_move_voice(&mut self, units: &[ObjectId]) {
        use crate::game_logic::audio_dispatch_impl::{UnitVoiceSlot, resolve_unit_voice_event};
        if self.game_logic.try_queue_picked_voice_move_upgraded(units) {
            return;
        }
        let has_salvage = units.iter().any(|&id| {
            self.game_logic
                .host_object(id)
                .and_then(|obj| {
                    resolve_unit_voice_event(&obj.template_name, UnitVoiceSlot::Salvage)
                })
                .is_some()
        });
        if has_salvage {
            self.game_logic
                .queue_picked_unit_voice(units, UnitVoiceSlot::Salvage);
        } else {
            self.play_context_move_voice(units);
        }
    }

    /// C++ AIGroup::groupMoveToPosition / computeIndividualDestination residual.
    ///
    /// Sort movers near→far to the click, take the nearest unit as the free-move
    /// "center", then offset each unit's goal by its (clamped) vector from that
    /// center — preserves relative formation instead of inventing a ring.

    /// C++ AIGroup player move/stop stealth residual: delay mood auto-acquire until
    /// unstealthed combat stealth units can cloak again.
    pub(super) fn apply_player_stealth_mood_delay(&mut self, unit_ids: &[ObjectId]) {
        // Wave 233: stealth mood delay via GameLogic authority API.
        // C++ GameLogicRandomValue(0, LOGICFRAMES_PER_SECOND) (AIGroup.cpp:2059).
        let now = self.game_logic.get_frame();
        for &unit_id in unit_ids {
            let skew = crate::game_logic::host_rng_residual::pure_logic_random_int(
                now.wrapping_add(unit_id.0),
                0,
                0,
                crate::game_logic::host_ai_path_combat_residual_wave105::LOGIC_FRAMES_PER_SECOND_RESIDUAL
                    as i32,
            ) as u32;
            let _ = self
                .game_logic
                .unit_command_apply_stealth_mood_delay(unit_id, now, skew);
        }
    }

    pub(crate) fn group_move_destinations(
        &self,
        units: &[ObjectId],
        destination: Vec3,
    ) -> Vec<(ObjectId, Vec3)> {
        if units.is_empty() {
            return Vec::new();
        }
        if units.len() == 1 {
            return vec![(units[0], destination)];
        }

        // Gather movable members with positions (skip dead / immobile).
        let mut movers: Vec<(ObjectId, Vec3, f32, u32, glam::Vec2, bool, bool)> =
            Vec::with_capacity(units.len());
        for &unit_id in units {
            let Some(obj) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !obj.is_alive() {
                continue;
            }
            if obj.is_kind_of(crate::game_logic::KindOf::Immobile)
                || obj.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            let radius = Self::bounding_circle_radius(obj);
            movers.push((
                unit_id,
                obj.get_position(),
                radius,
                obj.formation_id,
                obj.formation_offset,
                obj.is_kind_of(crate::game_logic::KindOf::Infantry),
                obj.is_kind_of(crate::game_logic::KindOf::Vehicle)
                    && !obj.is_kind_of(crate::game_logic::KindOf::Aircraft),
            ));
        }
        if movers.is_empty() {
            return units.iter().map(|&id| (id, destination)).collect();
        }
        if movers.len() == 1 {
            return vec![(movers[0].0, destination)];
        }

        // Shared non-zero formation id → C++ formation move offsets.
        let fid0 = movers[0].3;
        let is_formation = fid0 != 0 && movers.iter().all(|m| m.3 == fid0);
        if is_formation {
            return movers
                .into_iter()
                .map(|(id, _pos, _r, _fid, off, _inf, _veh)| {
                    (
                        id,
                        Vec3::new(destination.x + off.x, destination.y, destination.z + off.y),
                    )
                })
                .collect();
        }

        // C++ friend_moveInfantryToPos / friend_moveVehicleToPos residual:
        // each kind packs independently (infantry 3-col, vehicles 2-col). Mixed
        // selections still column-pack; leftovers (aircraft / short counts)
        // take the free-move path (AIGroup.cpp:1550-1553, :1637-1650).
        if let Some(column) = self.group_column_destinations(&movers, destination) {
            let packed: HashSet<ObjectId> = column.iter().map(|(id, _)| *id).collect();
            let leftover: Vec<_> = movers
                .iter()
                .filter(|m| !packed.contains(&m.0))
                .cloned()
                .collect();
            if leftover.is_empty() {
                return column;
            }
            let mut out = column;
            out.extend(Self::free_move_destinations(leftover, destination));
            return out;
        }

        Self::free_move_destinations(movers, destination)
    }

    /// C++ AIGroup::getMinMaxAndCenter formation return + groupMoveToPosition
    /// helipad / airborne-aircraft cancel (AIGroup.cpp:1543, :1575-1586).
    fn group_is_stamped_formation(&self, units: &[ObjectId]) -> bool {
        let mut fid0 = None;
        let mut count = 0u32;
        for &id in units {
            let Some(o) = self.game_logic.host_object(id) else {
                continue;
            };
            if !o.is_alive() || o.contained_by.is_some() {
                continue;
            }
            if o.is_kind_of(crate::game_logic::KindOf::Immobile)
                || o.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            let name = o.template_name.to_ascii_lowercase();
            let produced_at_helipad = name.contains("heli")
                || name.contains("chinook")
                || name.contains("comanche")
                || name.contains("helix");
            if produced_at_helipad {
                return false;
            }
            if o.is_kind_of(crate::game_logic::KindOf::Aircraft) && o.status.airborne_target {
                return false;
            }
            match fid0 {
                None => fid0 = Some(o.formation_id),
                Some(fid) if fid != o.formation_id => return false,
                _ => {}
            }
            count += 1;
        }
        matches!(fid0, Some(fid) if fid != 0) && count >= 2
    }

    /// C++ groupMoveToPosition free-move loop: nearest unit is the center,
    /// others keep a clamped offset from that center.
    fn free_move_destinations(
        mut movers: Vec<(ObjectId, Vec3, f32, u32, glam::Vec2, bool, bool)>,
        destination: Vec3,
    ) -> Vec<(ObjectId, Vec3)> {
        if movers.is_empty() {
            return Vec::new();
        }
        // Near-to-far vs goal (C++ SimpleObjectIterator ITER_SORTED_NEAR_TO_FAR).
        movers.sort_by(|a, b| {
            let da = (a.1.x - destination.x).hypot(a.1.z - destination.z);
            let db = (b.1.x - destination.x).hypot(b.1.z - destination.z);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });
        let center = movers[0].1;
        let mut out = Vec::with_capacity(movers.len());
        for (i, (unit_id, pos, radius, _fid, _off, _inf, _veh)) in movers.into_iter().enumerate() {
            let goal = if i == 0 {
                destination
            } else {
                let mut dx = pos.x - center.x;
                let mut dz = pos.z - center.z;
                let mut length = (dx * dx + dz * dz).sqrt();
                let max_length = 6.0 * radius;
                if length > max_length {
                    length = max_length;
                }
                if length > 0.001 {
                    let nlen = (dx * dx + dz * dz).sqrt().max(0.001);
                    dx = (dx / nlen) * length;
                    dz = (dz / nlen) * length;
                } else {
                    let angle = (i as f32) * 1.7;
                    dx = angle.cos() * radius * 0.5;
                    dz = angle.sin() * radius * 0.5;
                }
                Vec3::new(destination.x + dx, destination.y, destination.z + dz)
            };
            out.push((unit_id, goal));
        }
        out
    }

    /// C++ `AIGroup::computeIndividualDestination` with `isFormation=false`
    /// (`AIGroup.cpp:456-482`). Host Y-up: the group offset lives on XZ.
    fn compute_individual_destination(
        pos: Vec3,
        center: Vec3,
        group_dest: Vec3,
        bounding_radius: f32,
    ) -> Vec3 {
        let mut dx = pos.x - center.x;
        let mut dz = pos.z - center.z;
        let mut length = (dx * dx + dz * dz).sqrt();
        let max_length = 6.0 * bounding_radius.max(0.0);
        if length > max_length {
            length = max_length;
        }
        if length > 1e-6 {
            let nlen = (dx * dx + dz * dz).sqrt().max(1e-6);
            dx = (dx / nlen) * length;
            dz = (dz / nlen) * length;
        } else {
            dx = 0.0;
            dz = 0.0;
        }
        Vec3::new(group_dest.x + dx, group_dest.y, group_dest.z + dz)
    }

    /// C++ `groupMoveToPosition(addWaypoint=true)` dests: force
    /// `isFormation=false`, skip column packing, nearest member is the
    /// center, every member gets `computeIndividualDestination`
    /// (`AIGroup.cpp:1544-1553, :1674-1694`).
    fn group_add_waypoint_destinations(
        &self,
        units: &[ObjectId],
        destination: Vec3,
    ) -> Vec<(ObjectId, Vec3)> {
        let mut movers: Vec<(ObjectId, Vec3, f32)> = Vec::with_capacity(units.len());
        for &unit_id in units {
            let Some(obj) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !obj.is_alive()
                || !obj.can_move()
                || obj.status.disabled_held
                || obj.contained_by.is_some()
            {
                continue;
            }
            if obj.is_kind_of(crate::game_logic::KindOf::Immobile)
                || obj.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            movers.push((
                unit_id,
                obj.get_position(),
                Self::bounding_circle_radius(obj),
            ));
        }
        if movers.is_empty() {
            return Vec::new();
        }
        // Near-to-far vs goal (C++ SimpleObjectIterator ITER_SORTED_NEAR_TO_FAR).
        movers.sort_by(|a, b| {
            let da = (a.1.x - destination.x).hypot(a.1.z - destination.z);
            let db = (b.1.x - destination.x).hypot(b.1.z - destination.z);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });
        let center = movers[0].1;
        movers
            .into_iter()
            .map(|(id, pos, radius)| {
                (
                    id,
                    Self::compute_individual_destination(pos, center, destination, radius),
                )
            })
            .collect()
    }

    /// Retail GameData.ini `GroupMoveClickToGatherAreaFactor` residual (0.5).
    /// Leftover GlobalData ctor is 1.0 (C++ ctor) until INI applies; live
    /// uses the leftover residual so click-to-gather is the inner half bbox.
    /// C++ `ScaleRect2D` — no invented 20wu pad.
    fn group_move_click_to_gather_factor() -> f32 {
        crate::game_logic::host_ai_path_combat_residual_wave105::GROUP_MOVE_CLICK_TO_GATHER_FACTOR_RESIDUAL
    }

    /// True when destination lies inside the selected group's XZ bounding rect
    /// scaled by gather factor — C++ groupMoveToPosition tighten path
    /// (`AIGroup.cpp:1559-1608`). After ScaleRect2D the cell-area cap uses
    /// the scaled x-span twice (retail quirk) and only tightens when
    /// `cells < 2000`.
    pub(crate) fn should_tighten_group_move(&self, units: &[ObjectId], destination: Vec3) -> bool {
        let factor = Self::group_move_click_to_gather_factor();
        if factor <= 0.0 || units.len() < 2 {
            return false;
        }
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_z = f32::NEG_INFINITY;
        let mut count = 0u32;
        for &id in units {
            let Some(o) = self.game_logic.host_object(id) else {
                continue;
            };
            if !Self::group_ai_member_receives_move(o) {
                continue;
            }
            // Airborne fixed-wing: C++ disables tighten.
            if o.is_kind_of(crate::game_logic::KindOf::Aircraft)
                && o.status.airborne_target
                && !o.template_name.to_ascii_lowercase().contains("heli")
                && !o.template_name.to_ascii_lowercase().contains("chinook")
                && !o.template_name.to_ascii_lowercase().contains("comanche")
            {
                return false;
            }
            let p = o.get_position();
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_z = min_z.min(p.z);
            max_z = max_z.max(p.z);
            count += 1;
        }
        if count < 2 {
            return false;
        }
        // ScaleRect2D about center by gather factor (no 20wu pad).
        let cx = 0.5 * (min_x + max_x);
        let cz = 0.5 * (min_z + max_z);
        let hx = 0.5 * (max_x - min_x) * factor;
        let hz = 0.5 * (max_z - min_z) * factor;
        if !(destination.x >= cx - hx
            && destination.x <= cx + hx
            && destination.z >= cz - hz
            && destination.z <= cz + hz)
        {
            return false;
        }
        // C++ AIGroup.cpp:1602-1605 after ScaleRect2D mutates min/max:
        // dx=(max.x-min.x)/CELL; dy=(max.x-min.x)/CELL; cells=dx*dy; <2000.
        let cell = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
        let scaled_x = hx * 2.0;
        let dx = (scaled_x / cell) as i32;
        let cells = dx.saturating_mul(dx);
        cells < 2000
    }

    /// C++ AIGroup::groupTightenToPosition — near-to-far; helis get
    /// `getHelicopterOffset` slots (`AIGroup.cpp:1884-1898`).
    pub(crate) fn execute_tighten_to_position(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
    ) -> CommandResult {
        // Wave 232: tighten last-writes via GameLogic unit_command_tighten_to.
        if !destination.x.is_finite() || !destination.z.is_finite() {
            return CommandResult::InvalidLocation;
        }
        // Sort near-to-far (C++ SimpleObjectIterator ITER_SORTED_NEAR_TO_FAR).
        let mut movers: Vec<(ObjectId, f32, bool)> = Vec::new();
        for &unit_id in units {
            let Some(unit) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !Self::group_ai_member_receives_move(unit) {
                continue;
            }
            let p = unit.get_position();
            let dx = p.x - destination.x;
            let dz = p.z - destination.z;
            movers.push((
                unit_id,
                dx * dx + dz * dz,
                Self::is_produced_at_helipad(unit),
            ));
        }
        movers.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut any = false;
        let mut heli_idx = 0i32;
        for (unit_id, _, is_heli) in movers {
            let dest = if is_heli {
                let slot = Self::helicopter_offset(destination, heli_idx);
                heli_idx += 1;
                slot
            } else {
                destination
            };
            if self.game_logic.unit_command_tighten_to(unit_id, dest)
                || self.queue_group_move_goal(unit_id, dest)
            {
                any = true;
            }
        }
        if any {
            self.apply_player_stealth_mood_delay(units);
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::groupFollowWaypointPath / Exact / AsTeam residual.
    pub(crate) fn execute_follow_waypoint_path(
        &mut self,
        units: &[ObjectId],
        waypoints: &[Vec3],
        exact: bool,
        as_team: bool,
    ) -> CommandResult {
        // `exact` → assign_unit_path_exact (C++ AIFollowWaypointPathExactState).
        if waypoints.is_empty() {
            return CommandResult::InvalidLocation;
        }
        for wp in waypoints {
            if !wp.x.is_finite() || !wp.z.is_finite() {
                return CommandResult::InvalidLocation;
            }
        }

        // Collect movers + optional formation/group offsets (AsTeam residual).
        let mut movers: Vec<(ObjectId, Vec3, glam::Vec2)> = Vec::new();
        for &unit_id in units {
            let Some(unit) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() || !unit.can_move() {
                continue;
            }
            if unit.is_kind_of(crate::game_logic::KindOf::Immobile)
                || unit.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            movers.push((unit_id, unit.get_position(), unit.formation_offset));
        }
        if movers.is_empty() {
            return CommandResult::InvalidCommand;
        }

        // Group center from current positions.
        let (mut cx, mut cz) = (0.0f32, 0.0f32);
        for (_, pos, _) in &movers {
            cx += pos.x;
            cz += pos.z;
        }
        let n = movers.len() as f32;
        cx /= n;
        cz /= n;

        // Prefer stamped formation offsets when shared; else relative-to-center.
        let fid0 = self
            .game_logic
            .host_object(movers[0].0)
            .map(|o| o.formation_id)
            .unwrap_or(0);
        let use_formation = as_team
            && fid0 != 0
            && movers.iter().all(|(id, _, _)| {
                self.game_logic
                    .host_object(*id)
                    .map(|o| o.formation_id == fid0)
                    .unwrap_or(false)
            });

        // Near-to-far vs first waypoint.
        let first = waypoints[0];
        movers.sort_by(|a, b| {
            let da = (a.1.x - first.x).hypot(a.1.z - first.z);
            let db = (b.1.x - first.x).hypot(b.1.z - first.z);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });
        self.apply_group_desired_speed(units, as_team || use_formation);

        let mut any = false;
        for (unit_id, pos, form_off) in movers {
            let offset = if as_team {
                if use_formation {
                    form_off
                } else {
                    glam::Vec2::new(pos.x - cx, pos.z - cz)
                }
            } else {
                glam::Vec2::ZERO
            };

            let unit_wps: Vec<Vec3> = waypoints
                .iter()
                .map(|wp| Vec3::new(wp.x + offset.x, wp.y, wp.z + offset.y))
                .collect();
            let goal = *unit_wps.last().unwrap();
            let via = &unit_wps[..unit_wps.len().saturating_sub(1)];

            // Wave 233: waypoint-path prep via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_waypoint_path_prep(unit_id, as_team);
            // C++ AIFollowWaypointPathExact vs smoothed follow residual.
            let ok = if exact {
                self.game_logic.assign_unit_path_exact(unit_id, goal, via)
            } else {
                self.game_logic.assign_unit_path(unit_id, goal, via)
            };
            if ok {
                any = true;
            } else if self.path_to_goal_with_state(unit_id, goal, AIState::Moving) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup column path residual (infantry 3-col / vehicle group).
    /// Fail-closed: not full ground-path node following; destination-side pack only.
    pub(super) fn group_column_destinations(
        &self,
        movers: &[(ObjectId, Vec3, f32, u32, glam::Vec2, bool, bool)],
        destination: Vec3,
    ) -> Option<Vec<(ObjectId, Vec3)>> {
        use crate::game_logic::host_ai_path_combat_residual_wave105::{
            MIN_DISTANCE_FOR_GROUP_RESIDUAL, MIN_INFANTRY_FOR_GROUP_RESIDUAL,
            MIN_VEHICLES_FOR_GROUP_RESIDUAL,
        };

        // C++ groupMoveToPosition calls friend_moveInfantryToPos then
        // friend_moveVehicleToPos on the same member list (AIGroup.cpp:1550-1553).
        // Each pass packs only its kind (infantry 3-col, vehicles 2-col).
        let infantry: Vec<_> = movers.iter().filter(|m| m.5).cloned().collect();
        let vehicles: Vec<_> = movers.iter().filter(|m| m.6 && !m.5).cloned().collect();

        let mut out = Vec::new();
        if let Some(col) = Self::pack_column_kind(
            &infantry,
            destination,
            3,
            MIN_INFANTRY_FOR_GROUP_RESIDUAL,
            MIN_DISTANCE_FOR_GROUP_RESIDUAL,
        ) {
            out.extend(col);
        }
        if let Some(col) = Self::pack_column_kind(
            &vehicles,
            destination,
            2,
            MIN_VEHICLES_FOR_GROUP_RESIDUAL,
            MIN_DISTANCE_FOR_GROUP_RESIDUAL,
        ) {
            out.extend(col);
        }
        if out.is_empty() { None } else { Some(out) }
    }

    fn pack_column_kind(
        movers: &[(ObjectId, Vec3, f32, u32, glam::Vec2, bool, bool)],
        destination: Vec3,
        num_columns: i32,
        min_count: i32,
        min_distance: f32,
    ) -> Option<Vec<(ObjectId, Vec3)>> {
        let n = movers.len() as i32;
        if n < min_count {
            return None;
        }

        let mut center = Vec3::ZERO;
        for m in movers {
            center += m.1;
        }
        center /= movers.len() as f32;

        let mut dir_x = destination.x - center.x;
        let mut dir_z = destination.z - center.z;
        let dist = (dir_x * dir_x + dir_z * dir_z).sqrt();
        if dist < min_distance {
            return None;
        }
        dir_x /= dist;
        dir_z /= dist;
        // Perpendicular (C++ startVectorNormal: (-y, x) on XY → (-z, x) on XZ).
        let nx = -dir_z;
        let nz = dir_x;

        // Sort by projection on normal (C++ FAR_TO_NEAR on normal dot).
        let mut ordered: Vec<(ObjectId, Vec3, f32, f32)> = movers
            .iter()
            .map(|m| {
                let dx = m.1.x - center.x;
                let dz = m.1.z - center.z;
                let proj = dx * nx + dz * nz;
                (m.0, m.1, m.2, proj)
            })
            .collect();
        ordered.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));

        let half = num_columns / 2;
        let units_to_path = ordered.len() as i32;
        // C++: spacing uses path cell size; host residual ≈ average radius.
        let avg_r: f32 = ordered.iter().map(|o| o.2).sum::<f32>() / (ordered.len() as f32).max(1.0);
        let col_spacing = avg_r.max(8.0) * 1.25;
        let rank_spacing = avg_r.max(8.0) * 1.5;

        let mut out = Vec::with_capacity(ordered.len());
        for (cur_index, (id, _pos, _r, _proj)) in ordered.into_iter().enumerate() {
            let cur_index = cur_index as i32;
            // C++: divisor = (unitsToPath+1)/numColumns; columnDelta = 1 - curIndex/divisor
            let mut divisor = (units_to_path + 1) / num_columns;
            if divisor < 1 {
                divisor = 1;
            }
            let mut column_delta = 1 - (cur_index / divisor);
            if column_delta < -half {
                column_delta = -half;
            }
            if column_delta > half {
                column_delta = half;
            }
            // Rank depth along move direction (rows).
            let rank = cur_index / num_columns;
            let goal = Vec3::new(
                destination.x + nx * (column_delta as f32) * col_spacing
                    - dir_x * (rank as f32) * rank_spacing,
                destination.y,
                destination.z + nz * (column_delta as f32) * col_spacing
                    - dir_z * (rank as f32) * rank_spacing,
            );
            out.push((id, goal));
        }
        Some(out)
    }

    /// C++ AIGroup::groupAttackMoveToPosition residual.
    /// ableToAttack → attack-move path + maxShots; else plain move.
    pub(crate) fn execute_attack_move(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
        max_shots: i32,
    ) -> CommandResult {
        // C++ AIGroup::groupAttackMoveToPosition (`AIGroup.cpp:2260-2273`):
        // every member gets the identical `pos` — no column/formation spread.
        if !destination.x.is_finite() || !destination.z.is_finite() {
            return CommandResult::InvalidLocation;
        }
        let destination = self.clamp_group_waypoint(units, destination);
        let mut any = false;
        for &unit_id in units {
            if self
                .game_logic
                .host_object(unit_id)
                .is_some_and(|u| u.forbid_player_commands)
            {
                continue;
            }

            // C++ groupAttackMoveToPosition: any AI member. No can_move gate —
            // deployed artillery / turret structures still get attack-move.
            let (alive, can_attack) = match self.game_logic.host_object(unit_id) {
                Some(unit) => (unit.is_alive(), unit.can_attack() || unit.weapon.is_some()),
                None => continue,
            };
            if !alive {
                continue;
            }
            if can_attack {
                if self
                    .game_logic
                    .unit_command_attack_move_to_ex(unit_id, destination, max_shots)
                {
                    any = true;
                }
            } else if self
                .game_logic
                .unit_command_move_free(unit_id, destination, destination)
            {
                any = true;
            }
        }
        if any {
            // C++ MSG_DO_ATTACKMOVETO uses VoiceMove (`CommandXlat.cpp:384-412`).
            self.play_context_move_voice(units);
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_force_move(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
    ) -> CommandResult {
        // Wave 232: force-move via GameLogic unit_command_force_move_to.
        self.apply_group_desired_speed(units, self.group_is_stamped_formation(units));
        let goals = self.group_move_destinations(units, destination);
        if !self.group_is_stamped_formation(units) {
            let free_ids: Vec<ObjectId> = goals.iter().map(|(id, _)| *id).collect();
            self.dissolve_free_move_formation_stamps(&free_ids);
        }
        let mut moved: Vec<ObjectId> = Vec::new();
        for (unit_id, goal) in goals {
            if !self.game_logic.unit_command_force_move_to(unit_id, goal) {
                return CommandResult::InvalidCommand;
            }
            moved.push(unit_id);
        }
        self.apply_player_stealth_mood_delay(&moved);
        let crush = self.force_move_has_crush_target(&moved, destination);
        if crush {
            self.game_logic.queue_picked_unit_voice(
                &moved,
                crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::Crush,
            );
        } else {
            self.play_context_move_voice(&moved);
        }
        CommandResult::Success
    }

    pub(crate) fn execute_add_waypoint(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
    ) -> CommandResult {
        // C++ groupMoveToPosition(addWaypoint=true): clamp first, then
        // isFormation=false / skip column packing / clear formation /
        // computeIndividualDestination. Held/immobile/no-AI members are
        // skipped; the rest still append (AIGroup.cpp:1528-1727).
        // issueMoveToLocationCommand plays VoiceMove for MSG_ADD_WAYPOINT
        // too, but pickAndPlay skips units already moving in waypoint mode.
        let destination = self.clamp_group_waypoint(units, destination);
        let goals = self.group_add_waypoint_destinations(units, destination);
        let mut moved: Vec<ObjectId> = Vec::new();
        let mut voice: Vec<ObjectId> = Vec::new();
        for (unit_id, goal) in goals {
            let Some(obj) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            let was_moving = obj.is_effectively_moving();
            let goal = self
                .game_logic
                .adjust_group_member_goal(unit_id, goal, destination);
            let _ = self
                .game_logic
                .unit_command_set_formation(unit_id, 0, glam::Vec2::ZERO);
            if !self.game_logic.append_unit_waypoint(unit_id, goal) {
                continue;
            }
            if !was_moving {
                voice.push(unit_id);
            }
            moved.push(unit_id);
            debug!("Added waypoint for unit {} at {:?}", unit_id.0, goal);
        }
        self.apply_player_stealth_mood_delay(&moved);
        if !voice.is_empty() {
            self.play_context_move_voice(&voice);
        }
        CommandResult::Success
    }

    /// C++ AIGroup::friend_moveFormationToPos residual.
    /// Paths each formation member to dest + stamped offset.
    pub(crate) fn execute_move_formation_to_position(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
    ) -> CommandResult {
        // Wave 232: formation move via unit_command_force_move_to.
        if !destination.x.is_finite() || !destination.z.is_finite() {
            return CommandResult::InvalidLocation;
        }
        // Ensure formation stamps exist.
        let need_stamp = {
            let fid0 = units
                .first()
                .and_then(|id| self.game_logic.host_object(*id))
                .map(|o| o.formation_id)
                .unwrap_or(0);
            fid0 == 0
                || !units.iter().all(|&id| {
                    self.game_logic
                        .host_object(id)
                        .map(|o| o.formation_id == fid0 && fid0 != 0)
                        .unwrap_or(false)
                })
        };
        if need_stamp {
            let _ = self.execute_create_formation(units);
        }
        self.apply_group_desired_speed(units, true);
        let goals = self.group_move_destinations(units, destination);
        let mut any = false;
        for (unit_id, goal) in goals {
            if self.game_logic.unit_command_force_move_to(unit_id, goal)
                || self.path_to_goal_with_state(unit_id, goal, AIState::Moving)
            {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::groupFollowPath residual (empty body in retail) —
    /// host uses non-exact waypoint follow.
    pub(crate) fn execute_follow_path(
        &mut self,
        units: &[ObjectId],
        path: &[Vec3],
    ) -> CommandResult {
        self.execute_follow_waypoint_path(units, path, false, false)
    }

    pub(crate) fn execute_patrol(&mut self, units: &[ObjectId]) -> CommandResult {
        // Wave 232: patrol last-writes via GameLogic unit_command_patrol.
        let mut any = false;
        for &unit_id in units {
            if self
                .game_logic
                .host_object(unit_id)
                .is_some_and(|u| u.forbid_player_commands)
            {
                continue;
            }
            if self.game_logic.unit_command_patrol(unit_id) {
                any = true;
            }
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(crate) fn execute_scatter(&mut self, units: &[ObjectId]) -> CommandResult {
        // Wave 232: scatter last-writes via GameLogic unit_command_move_to_moving.
        // C++ AIGroup::groupScatter — far-to-near from group center, push out by
        // 4 * bounding radius along the unit→center vector (host XZ plane).
        let mut movers: Vec<(ObjectId, Vec3, f32)> = Vec::new();
        for &unit_id in units {
            let Some(unit) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !Self::group_ai_member_receives_move(unit) {
                continue;
            }
            let pos = unit.get_position();
            let radius = Self::bounding_circle_radius(unit);
            movers.push((unit_id, pos, radius));
        }
        if movers.is_empty() {
            return CommandResult::InvalidCommand;
        }

        let mut center = Vec3::ZERO;
        for (_, pos, _) in &movers {
            center += *pos;
        }
        center /= movers.len() as f32;

        movers.sort_by(|a, b| {
            let da = (a.1.x - center.x).hypot(a.1.z - center.z);
            let db = (b.1.x - center.x).hypot(b.1.z - center.z);
            db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut any = false;
        let mut center_nudge = center;
        for (unit_id, pos, radius) in movers {
            center_nudge.x -= 0.01;
            let mut dx = pos.x - center_nudge.x;
            let mut dz = pos.z - center_nudge.z;
            let len = (dx * dx + dz * dz).sqrt();
            if len > 0.001 {
                dx /= len;
                dz /= len;
            } else {
                dx = 1.0;
                dz = 0.0;
            }
            let push = 4.0 * radius;
            let dest = Vec3::new(pos.x + dx * push, pos.y, pos.z + dz * push);
            if self.queue_group_move_goal(unit_id, dest) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }
}
