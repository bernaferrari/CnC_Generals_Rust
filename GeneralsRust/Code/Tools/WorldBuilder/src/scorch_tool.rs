//! C++ `ScorchTool.cpp` pick + mouseDown (no MFC options dialog).
//!
//! Tight hit 0.5 * MAP_XY_FACTOR, then loose 1.5 * MAP_XY_FACTOR. Miss creates
//! a `Scorch` MapObject with `originalOwner=team`, `objectRadius`, `scorchType`.

use game_engine::map_object::{Coord3D, MAP_XY_FACTOR, MapObject};

/// C++ `DEFAULT_SCORCHMARK_RADIUS`.
pub const DEFAULT_SCORCHMARK_RADIUS: f32 = 20.0;
/// C++ `Scorches::SCORCH_1`.
pub const SCORCH_1: i32 = 0;
/// C++ `NEUTRAL_TEAM_INTERNAL_STR` (`mapobjectprops.cpp`).
pub const NEUTRAL_TEAM_INTERNAL_STR: &str = "team";

fn xy_distance(a: &Coord3D, b: &Coord3D) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

/// C++ `ScorchTool::pickScorch` — tight pass then loose pass, first match.
pub fn pick_scorch(objects: &[MapObject], loc: Coord3D) -> Option<usize> {
    let tight = 0.5 * MAP_XY_FACTOR;
    for (index, object) in objects.iter().enumerate() {
        if object.is_scorch() && xy_distance(object.get_location(), &loc) < tight {
            return Some(index);
        }
    }
    let loose = 1.5 * MAP_XY_FACTOR;
    for (index, object) in objects.iter().enumerate() {
        if object.is_scorch() && xy_distance(object.get_location(), &loc) < loose {
            return Some(index);
        }
    }
    None
}

/// C++ `WbView::snapPoint` residual: snap XY to MAP_XY_FACTOR cells.
pub fn snap_doc_point(x: f32, y: f32) -> (f32, f32) {
    (
        (x / MAP_XY_FACTOR).round() * MAP_XY_FACTOR,
        (y / MAP_XY_FACTOR).round() * MAP_XY_FACTOR,
    )
}

/// C++ `ScorchTool::mouseDown` TRACK_L: select existing scorch or place a new one.
pub fn mouse_down_scorch(
    objects: &mut Vec<MapObject>,
    loc: Coord3D,
    scorch_size: f32,
    scorch_type: i32,
) -> usize {
    if let Some(index) = pick_scorch(objects, loc.clone()) {
        for (i, object) in objects.iter_mut().enumerate() {
            object.set_selected(i == index);
        }
        return index;
    }
    let (sx, sy) = snap_doc_point(loc.x, loc.y);
    for object in objects.iter_mut() {
        object.set_selected(false);
    }
    let mut created = MapObject::new(Coord3D::new(sx, sy, loc.z), "Scorch", 0.0, 0);
    created.set_selected(true);
    created.set_is_scorch();
    created
        .get_properties_mut()
        .insert("originalOwner".into(), NEUTRAL_TEAM_INTERNAL_STR.into());
    created
        .get_properties_mut()
        .insert("objectRadius".into(), scorch_size.to_string());
    created
        .get_properties_mut()
        .insert("scorchType".into(), scorch_type.to_string());
    objects.push(created);
    objects.len() - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_scorch_tight_then_loose_like_cpp() {
        let mut near = MapObject::new(Coord3D::new(100.0, 100.0, 0.0), "Scorch", 0.0, 0);
        near.set_is_scorch();
        let far = MapObject::new(Coord3D::new(0.0, 0.0, 0.0), "Tank", 0.0, 0);
        let objects = vec![far, near];
        assert_eq!(
            pick_scorch(&objects, Coord3D::new(102.0, 100.0, 0.0)),
            Some(1),
            "within 0.5*MAP_XY_FACTOR (5wu) must hit"
        );
        assert_eq!(
            pick_scorch(&objects, Coord3D::new(112.0, 100.0, 0.0)),
            Some(1),
            "within 1.5*MAP_XY_FACTOR (15wu) loose pass"
        );
        assert_eq!(pick_scorch(&objects, Coord3D::new(200.0, 200.0, 0.0)), None);
    }

    #[test]
    fn mouse_down_places_scorch_with_cpp_defaults() {
        let mut objects = Vec::new();
        let idx = mouse_down_scorch(
            &mut objects,
            Coord3D::new(14.0, 26.0, 3.0),
            DEFAULT_SCORCHMARK_RADIUS,
            SCORCH_1,
        );
        assert_eq!(idx, 0);
        assert_eq!(objects.len(), 1);
        assert!(objects[0].is_scorch());
        assert!(objects[0].is_selected());
        assert_eq!(objects[0].object_name, "Scorch");
        assert_eq!(objects[0].get_location().x, 10.0);
        assert_eq!(objects[0].get_location().y, 30.0);
        assert_eq!(
            objects[0]
                .get_properties()
                .get("originalOwner")
                .map(String::as_str),
            Some(NEUTRAL_TEAM_INTERNAL_STR)
        );
        assert_eq!(
            objects[0]
                .get_properties()
                .get("objectRadius")
                .map(String::as_str),
            Some("20")
        );
        assert_eq!(
            objects[0]
                .get_properties()
                .get("scorchType")
                .map(String::as_str),
            Some("0")
        );
    }

    #[test]
    fn mouse_down_selects_existing_scorch_instead_of_placing() {
        let mut objects = Vec::new();
        let first = mouse_down_scorch(
            &mut objects,
            Coord3D::new(50.0, 50.0, 0.0),
            DEFAULT_SCORCHMARK_RADIUS,
            SCORCH_1,
        );
        let again = mouse_down_scorch(&mut objects, Coord3D::new(52.0, 50.0, 0.0), 40.0, 2);
        assert_eq!(first, 0);
        assert_eq!(again, 0);
        assert_eq!(objects.len(), 1);
        assert!(objects[0].is_selected());
    }
}
