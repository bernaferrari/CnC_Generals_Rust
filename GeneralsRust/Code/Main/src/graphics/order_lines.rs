use crate::game_logic::ObjectId;
use crate::presentation_frame::RenderableObject;
use glam::Vec3;
use std::collections::HashMap;

pub(super) type InterpPoint<'a> = dyn Fn(ObjectId, Vec3) -> Vec3 + 'a;

pub(super) fn pack_order_lines(
    objects: &[RenderableObject],
    show_move_lines: bool,
    show_attack_lines: bool,
    interp: &InterpPoint<'_>,
) -> Vec<f32> {
    let mut vertices = Vec::new();
    if show_move_lines {
        append_move_lines(&mut vertices, objects, interp);
    }
    if show_attack_lines {
        append_attack_lines(&mut vertices, objects, interp);
    }
    vertices
}

fn append_move_lines(
    vertices: &mut Vec<f32>,
    objects: &[RenderableObject],
    interp: &InterpPoint<'_>,
) {
    let color = [0.2, 0.9, 1.0, 0.85];
    for unit in objects {
        if unit.destroyed || unit.drawable_hidden {
            continue;
        }
        if let Some(destination) = unit.move_destination {
            push_segment(vertices, interp(unit.id, unit.position), destination, color);
        }
        for waypoints in unit.path_waypoints.windows(2) {
            push_segment(vertices, waypoints[0], waypoints[1], color);
        }
    }
}

fn append_attack_lines(
    vertices: &mut Vec<f32>,
    objects: &[RenderableObject],
    interp: &InterpPoint<'_>,
) {
    let positions: HashMap<_, _> = objects
        .iter()
        .filter(|unit| !unit.destroyed && !unit.drawable_hidden)
        .map(|unit| (unit.id, unit.position))
        .collect();

    let color = [1.0, 0.25, 0.15, 0.9];
    for unit in objects {
        if unit.destroyed || unit.drawable_hidden {
            continue;
        }
        if let Some(target) = unit.attack_target {
            if let Some(&position) = positions.get(&target) {
                push_segment(
                    vertices,
                    interp(unit.id, unit.position),
                    interp(target, position),
                    color,
                );
            }
        }
    }
}

fn push_segment(vertices: &mut Vec<f32>, start: Vec3, end: Vec3, color: [f32; 4]) {
    for point in [start, end] {
        vertices.extend_from_slice(&point.to_array());
        vertices.extend_from_slice(&color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::presentation_frame::PresentationFrame;

    #[test]
    fn empty_orders_emit_no_vertices() {
        let frame = PresentationFrame::try_build_from_logic(&mut GameLogic::new(), 0).unwrap();
        assert!(pack_order_lines(&frame.objects, true, true, &|_, p| p).is_empty());
    }

    #[test]
    fn move_line_interpolates_unit_and_keeps_destination_and_waypoints_fixed() {
        let mut logic = unit_fixture();
        let id = logic
            .create_object("OrderLineUnit", Team::USA, Vec3::ZERO)
            .unwrap();
        let unit = logic.host_object_mut(id).unwrap();
        unit.movement.target_position = Some(Vec3::new(30.0, 0.0, 10.0));
        unit.movement.set_path_coords(vec![
            Vec3::ZERO,
            Vec3::new(15.0, 0.0, 5.0),
            Vec3::new(30.0, 0.0, 10.0),
        ]);

        let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
        let vertices = pack_order_lines(&frame.objects, true, false, &|_, p| p + Vec3::X * 5.0);
        assert_eq!(
            vertices,
            [
                5.0, 0.0, 0.0, 0.2, 0.9, 1.0, 0.85, 30.0, 0.0, 10.0, 0.2, 0.9, 1.0, 0.85, 0.0, 0.0,
                0.0, 0.2, 0.9, 1.0, 0.85, 15.0, 0.0, 5.0, 0.2, 0.9, 1.0, 0.85, 15.0, 0.0, 5.0, 0.2,
                0.9, 1.0, 0.85, 30.0, 0.0, 10.0, 0.2, 0.9, 1.0, 0.85,
            ]
        );
        assert!(pack_order_lines(&frame.objects, false, false, &|_, p| p).is_empty());
    }

    #[test]
    fn attack_line_interpolates_visible_units() {
        let mut logic = unit_fixture();
        let attacker = logic
            .create_object("OrderLineUnit", Team::USA, Vec3::ZERO)
            .unwrap();
        let target = logic
            .create_object("OrderLineUnit", Team::USA, Vec3::X * 20.0)
            .unwrap();
        logic.host_object_mut(attacker).unwrap().target = Some(target);

        let mut frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
        let interp = |id, p| {
            if id == attacker {
                p + Vec3::X * 5.0
            } else {
                p - Vec3::X * 5.0
            }
        };
        assert_eq!(
            pack_order_lines(&frame.objects, false, true, &interp),
            [
                5.0, 0.0, 0.0, 1.0, 0.25, 0.15, 0.9, 15.0, 0.0, 0.0, 1.0, 0.25, 0.15, 0.9,
            ]
        );

        let visible_target = frame
            .objects
            .iter_mut()
            .find(|unit| unit.id == target)
            .unwrap();
        visible_target.drawable_hidden = true;
        assert!(pack_order_lines(&frame.objects, false, true, &interp).is_empty());
    }

    fn unit_fixture() -> GameLogic {
        let mut logic = GameLogic::new();
        let mut template = ThingTemplate::new("OrderLineUnit");
        template.set_health(50.0);
        template.add_kind_of(KindOf::Infantry);
        logic.templates.insert("OrderLineUnit".into(), template);
        logic
    }
}
