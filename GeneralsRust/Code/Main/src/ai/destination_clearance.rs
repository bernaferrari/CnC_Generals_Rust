use super::*;

/// Leftover `iterateCellsAlongLine` Bresenham (AIPathfind.cpp / tall_buildings.rs).
fn dest_line_cells(sx: i32, sy: i32, ex: i32, ey: i32) -> Vec<(i32, i32)> {
    if sx == ex && sy == ey {
        return vec![(sx, sy)];
    }
    let mut out = Vec::new();
    let delta_x = (ex - sx).abs();
    let delta_y = (ey - sy).abs();
    let mut x = sx;
    let mut y = sy;
    let (mut xinc1, mut xinc2) = if ex >= sx { (1, 1) } else { (-1, -1) };
    let (mut yinc1, mut yinc2) = if ey >= sy { (1, 1) } else { (-1, -1) };
    let (den, mut num, numadd, numpixels);
    if delta_x >= delta_y {
        xinc1 = 0;
        yinc2 = 0;
        den = delta_x;
        num = delta_x / 2;
        numadd = delta_y;
        numpixels = delta_x;
    } else {
        xinc2 = 0;
        yinc1 = 0;
        den = delta_y;
        num = delta_y / 2;
        numadd = delta_x;
        numpixels = delta_y;
    }
    for _ in 0..=numpixels {
        out.push((x, y));
        num += numadd;
        if den != 0 && num >= den {
            num -= den;
            x += xinc1;
            y += yinc1;
            out.push((x, y));
        }
        x += xinc2;
        y += yinc2;
    }
    out
}

impl GameLogic {
    /// C++ `Pathfinder::moveAlliesAwayFromDestination` (AIPathfind.cpp:6911-6922).
    /// Factory-exit dest-line: leftover occupancy first, then host idle allies.
    pub fn move_allies_away_from_destination(&mut self, unit_id: ObjectId, destination: Vec3) {
        let Some(unit) = self.objects.get(&unit_id) else {
            return;
        };
        let from = unit.get_position();
        let ignore = unit.ignored_obstacle_id;
        let mover_player = unit.owner_player_id.unwrap_or(unit.team as u32);
        let mover_team = unit.team;

        let leftover_from = gamelogic::common::Coord3D::new(from.x, from.z, from.y);
        let leftover_dest =
            gamelogic::common::Coord3D::new(destination.x, destination.z, destination.y);
        let ai_store = gamelogic::ai::the_ai(); if let Ok(ai) = ai_store.read() {
            if let Some(pf) = ai.pathfinder() {
                if let Ok(pf) = pf.read() {
                    let _ = pf.move_allies_away_from_destination_for(
                        unit_id.0,
                        &leftover_from,
                        &leftover_dest,
                    );
                }
            }
        }

        let from_c = self.pathfinding_system.grid.world_to_grid(from);
        let to_c = self.pathfinding_system.grid.world_to_grid(destination);
        let cells = dest_line_cells(from_c.x, from_c.y, to_c.x, to_c.y);

        let mut nudged = Vec::new();
        for (cx, cy) in cells {
            for obj in self.objects.values() {
                if obj.id == unit_id || !obj.is_alive() || ignore == Some(obj.id) {
                    continue;
                }
                if obj.is_kind_of(KindOf::Structure) || obj.is_kind_of(KindOf::Immobile) {
                    continue;
                }
                let other_p = obj.owner_player_id.unwrap_or(obj.team as u32);
                if other_p != mover_player && obj.team != mover_team {
                    continue;
                }
                if !obj.movement.path.is_empty()
                    || obj.movement.velocity.length_squared() > 0.25
                    || obj.status.attacking
                    || obj.status.using_ability
                    || obj.deploy_style.as_ref().is_some_and(|d| d.is_busy())
                {
                    continue;
                }
                let oc = self
                    .pathfinding_system
                    .grid
                    .world_to_grid(obj.get_position());
                if oc.x == cx && oc.y == cy && !nudged.contains(&obj.id) {
                    nudged.push(obj.id);
                }
            }
        }

        for ally in nudged {
            if let Some(obj) = self.objects.get_mut(&ally) {
                obj.ai_move_away_from_unit(unit_id, from);
            }
        }
    }
}
