//! Unchanged public API witness also executable against the pre-region implementation.
use super::*;

#[test]
fn public_region_iteration_preserves_factory_constructor_roster_and_inclusive_edges() {
    let mut client = GameClient::new().unwrap();
    let template = ThingTemplate::new();
    let mut all = Vec::new();
    let mut inside = Vec::new();
    for ordinal in 0..64 {
        let id = client.create_drawable_from_template(&template).unwrap();
        let point = match ordinal % 4 {
            0 => Vector3::new(-2., -3., -4.),
            1 => Vector3::new(5., 6., 7.),
            2 => Vector3::new(0., 0., 0.),
            _ => Vector3::new(5.001, 0., 0.),
        };
        client
            .find_drawable_by_id_mut(id)
            .unwrap()
            .set_position(point);
        all.push(id);
        if ordinal % 4 != 3 {
            inside.push(id);
        }
    }
    all.reverse();
    inside.reverse();
    let region = Region3D {
        lo: crate::system::Coord3D::new(-2., -3., -4.),
        hi: crate::system::Coord3D::new(5., 6., 7.),
    };
    for _ in 0..2 {
        let mut observed = Vec::new();
        client
            .iterate_drawables_in_region(None, |drawable| observed.push(drawable.get_id()))
            .unwrap();
        assert_eq!(
            observed, all,
            "CPP public region walk follows constructor-prepended list"
        );
        observed.clear();
        client
            .iterate_drawables_in_region(Some(&region), |drawable| observed.push(drawable.get_id()))
            .unwrap();
        assert_eq!(
            observed, inside,
            "inclusive faces/corners preserve filtered roster order"
        );
    }
}
