use super::*;
#[derive(Clone, Copy)]
struct Record {
    cost: u32,
    parent: Option<Cell>,
    closed: bool,
    order: u64,
    total: u32,
}
fn remaining(a: Cell, b: Cell) -> u32 {
    let dx = (a.pos.x - b.pos.x).unsigned_abs();
    let dy = (a.pos.y - b.pos.y).unsigned_abs();
    10 * dx.max(dy) + 5 * dx.min(dy)
}
fn admit(
    records: &mut HashMap<Cell, Record>,
    open: &mut BinaryHeap<std::cmp::Reverse<(u32, u64, i32, i32, u8)>>,
    serial: &mut u64,
    cell: Cell,
    parent: Option<Cell>,
    cost: u32,
    total: u32,
) {
    let order = *serial;
    *serial += 1;
    records.insert(
        cell,
        Record {
            cost,
            parent,
            closed: false,
            order,
            total,
        },
    );
    open.push(std::cmp::Reverse((
        total, order, cell.pos.x, cell.pos.y, cell.layer,
    )));
}
pub(super) fn find(
    grid: &PathfindingGrid,
    start: Cell,
    goal: Cell,
    objects: &HashMap<ObjectId, Object>,
    passable: &impl Fn(GridPos) -> bool,
    cleanup_cells: &mut usize,
) -> Option<(Vec<Cell>, u32)> {
    let mut records = HashMap::new();
    let mut open = BinaryHeap::new();
    let mut serial = 0;
    admit(
        &mut records,
        &mut open,
        &mut serial,
        start,
        None,
        0,
        remaining(start, goal),
    );
    while let Some(std::cmp::Reverse((_, order, x, y, layer))) = open.pop() {
        let current = Cell {
            pos: GridPos::new(x, y),
            layer,
        };
        let record = *records.get(&current)?;
        if record.closed || record.order != order {
            continue;
        }
        if current == goal {
            let mut result = vec![goal];
            let mut p = goal;
            while let Some(parent) = records.get(&p)?.parent {
                result.push(parent);
                p = parent;
            }
            result.reverse();
            // C++ releases the popped goal separately, then counts the cells
            // left on its open/closed lists (cleanOpenAndClosedLists:4787).
            *cleanup_cells = records.len().saturating_sub(1);
            return Some((result, record.cost));
        }
        records.get_mut(&current)?.closed = true;
        if let Some(next_layer) =
            grid.connect_layer_of(current.pos, PathfindLayerEnum::from_u32(layer as u32))
        {
            let next = grid.group_cell(current.pos, next_layer as u8);
            if !records.contains_key(&next) {
                admit(
                    &mut records,
                    &mut open,
                    &mut serial,
                    next,
                    Some(current),
                    record.cost,
                    record.total,
                );
            }
        }
        let mut previous = current;
        for pos in line_cells(current.pos, goal.pos).into_iter().skip(1) {
            let next = grid.group_cell(pos, layer);
            if !grid.is_valid_pos(pos) {
                break;
            }
            if records.contains_key(&next) || grid.group_clearance(next, 6, objects) != 6 {
                break;
            }
            let cost = records[&previous].cost + 5;
            admit(
                &mut records,
                &mut open,
                &mut serial,
                next,
                Some(previous),
                cost,
                cost + remaining(next, goal),
            );
            previous = next;
        }
        let mut flags = [false; 8];
        let mut carried_cost = None;
        for (i, (dx, dy)) in [
            (1, 0),
            (0, 1),
            (-1, 0),
            (0, -1),
            (1, 1),
            (-1, 1),
            (-1, -1),
            (1, -1),
        ]
        .into_iter()
        .enumerate()
        {
            let pos = GridPos::new(x + dx, y + dy);
            if !grid.is_valid_pos(pos) {
                continue;
            }
            if layer == PathfindLayerEnum::Ground as u8
                && !passable(pos)
                && ![(3, 3), (-3, 3), (3, -3), (-3, -3)]
                    .into_iter()
                    .any(|(a, b)| passable(GridPos::new(pos.x + a, pos.y + b)))
            {
                continue;
            }
            let next = grid.group_cell(pos, layer);
            if next != goal {
                if i >= 4 && !flags[i - 4] && !flags[(i - 3) % 4] {
                    continue;
                }
                if grid.resolved_cell_type(PathfindLayerEnum::from_u32(next.layer as u32), pos)
                    != PathfindCellType::Clear
                    || (next.layer == PathfindLayerEnum::Ground as u8 && grid.is_pinched(pos))
                {
                    continue;
                }
                flags[i] = true;
            }
            let mut cost = record.cost + if dx == 0 || dy == 0 { 10 } else { 14 };
            if let Some(parent) = record.parent {
                let old = (parent.pos.x - x, parent.pos.y - y);
                let new = (-dx, -dy);
                if old != new {
                    let dot = old.0 * new.0 + old.1 * new.1;
                    cost += if dot > 0 {
                        4
                    } else if dot == 0 {
                        8
                    } else {
                        16
                    };
                }
            }
            if next != goal {
                cost +=
                    (0.6f32 * ((6 - grid.group_clearance(next, 6, objects)) * 10) as f32) as u32;
            }
            if next == goal {
                // The original retains the preceding neighbor's computed cost.
                // When no preceding neighbor assigned it, C++ reads an
                // uninitialized unsigned local: use this actual step's cost
                // as the defined Rust completion of that unspecified case.
                cost = carried_cost.unwrap_or(cost);
            } else {
                carried_cost = Some(cost);
            }
            if records.get(&next).is_some_and(|r| r.cost <= cost) {
                continue;
            }
            admit(
                &mut records,
                &mut open,
                &mut serial,
                next,
                Some(current),
                cost,
                cost + remaining(next, goal),
            );
        }
    }
    *cleanup_cells = records.len();
    None
}
