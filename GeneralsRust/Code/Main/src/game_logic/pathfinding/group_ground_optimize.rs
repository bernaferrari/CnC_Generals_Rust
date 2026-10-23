use super::*;

#[derive(Clone, Copy)]
struct Node {
    position: Vec3,
    layer: u8,
}

pub(super) fn build(
    grid: &PathfindingGrid,
    from: Vec3,
    cells: &[Cell],
    objects: HostObjectRead<'_>,
    ground_source: &PathfindingGround<'_>,
) -> Vec<Vec3> {
    let mut reversed: Vec<Node> = Vec::new();
    for cell in cells.iter().skip(1).rev() {
        if let Some(previous) = reversed.last_mut() {
            if grid.world_to_grid(previous.position) == cell.pos {
                previous.layer = if cell.layer == PathfindLayerEnum::Ground as u8 {
                    previous.layer
                } else {
                    cell.layer
                };
                continue;
            }
        }
        let mut position = grid.adjust_coord_to_cell(cell.pos, false, ground_source);
        position.y = grid.layer_world_height(
            cell.pos,
            PathfindLayerEnum::from_u32(cell.layer as u32),
            position.x,
            position.z,
            ground_source,
        );
        reversed.push(Node {
            position,
            layer: cell.layer,
        });
    }
    if reversed.is_empty() {
        let cell = cells[0];
        let mut position = grid.adjust_coord_to_cell(cell.pos, false, ground_source);
        position.y = grid.layer_world_height(
            cell.pos,
            PathfindLayerEnum::from_u32(cell.layer as u32),
            position.x,
            position.z,
            ground_source,
        );
        reversed.push(Node {
            position,
            layer: cell.layer,
        });
    }
    reversed.reverse();
    if reversed[0].position.x != from.x || reversed[0].position.z != from.z {
        reversed.insert(
            0,
            Node {
                position: from,
                layer: cells[0].layer,
            },
        );
    }
    let nodes = reversed;
    let mut next = vec![None; nodes.len()];
    let mut anchor = 0;
    while anchor + 1 < nodes.len() {
        let mut layer = nodes[anchor].layer;
        let mut current_layer = layer;
        let mut far = anchor + 1;
        let mut count = 0;
        while far + 1 < nodes.len() {
            count += 1;
            if current_layer == PathfindLayerEnum::Ground as u8 {
                if nodes[far].layer != current_layer {
                    layer = nodes[far].layer;
                    current_layer = layer;
                    if count > 3 {
                        break;
                    }
                }
            } else if nodes[far + 1].layer != current_layer && count > 3 {
                break;
            }
            current_layer = nodes[far].layer;
            far += 1;
        }
        let selected = (anchor + 1..=far)
            .rev()
            .find(|&end| {
                let mut previous = None;
                let los = line_cells(
                    grid.world_to_grid(nodes[anchor].position),
                    grid.world_to_grid(nodes[end].position),
                )
                .into_iter()
                .all(|pos| {
                    if !grid.is_valid_pos(pos) {
                        return true;
                    }
                    let pass = grid.group_clearance(Cell { pos, layer }, 6, objects) == 6
                        || previous.is_some() && layer != PathfindLayerEnum::Ground as u8;
                    previous = Some(pos);
                    pass
                });
                los || straight_steps(&nodes, anchor, end)
            })
            .unwrap_or(anchor + 1);
        next[anchor] = Some(selected);
        anchor = selected;
    }
    // Original optimized-chain iteration removes one short jig/jog at each
    // visited anchor, without repeatedly collapsing the same anchor.
    anchor = 0;
    while let Some(middle) = next[anchor] {
        if let Some(after) = next[middle] {
            let d = nodes[middle].position - nodes[anchor].position;
            if d.x * d.x + d.z * d.z < grid.grid_size * grid.grid_size * 3.9 {
                next[anchor] = Some(after);
            }
        }
        anchor = next[anchor].unwrap();
    }
    let mut result = Vec::new();
    anchor = 0;
    loop {
        result.push(nodes[anchor].position);
        match next[anchor] {
            Some(index) => anchor = index,
            None => break,
        }
    }
    result
}
fn straight_steps(nodes: &[Node], start: usize, end: usize) -> bool {
    let delta = nodes[end].position - nodes[start].position;
    let mut dx = delta.x as i32;
    let mut dy = delta.z as i32;
    let mut pass = false;
    // Preserve the original mutable integer temporaries and reverse scan;
    // later tests intentionally observe values left by an earlier scan.
    if dx == 0 {
        pass = true;
        for index in (start + 1..end).rev() {
            dx = (nodes[index + 1].position.x - nodes[index].position.x) as i32;
            if dx != 0 {
                pass = false;
            }
        }
    }
    if dy == 0 {
        pass = true;
        for index in (start + 1..end).rev() {
            dy = (nodes[index + 1].position.z - nodes[index].position.z) as i32;
            if dy != 0 {
                pass = false;
            }
        }
    }
    if dx == dy {
        pass = true;
        for index in (start + 1..end).rev() {
            dx = (nodes[index + 1].position.x - nodes[index].position.x) as i32;
            dy = (nodes[index + 1].position.z - nodes[index].position.z) as i32;
            if dy != dx {
                pass = false;
            }
        }
    }
    if dx == -dy {
        pass = true;
        for index in (start + 1..end).rev() {
            dx = (nodes[index + 1].position.x - nodes[index].position.x) as i32;
            dy = (nodes[index + 1].position.z - nodes[index].position.z) as i32;
            if dy != -dx {
                pass = false;
            }
        }
    }
    pass
}
