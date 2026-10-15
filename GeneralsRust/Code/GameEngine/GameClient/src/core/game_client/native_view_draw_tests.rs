use super::*;
use crate::drawable::Drawable;

fn view(lo: [f32; 3], hi: [f32; 3]) -> NativeDrawView {
    NativeDrawView {
        region: ViewAabb { lo, hi },
    }
}
fn objectless(client: &mut GameClient, position: [f32; 3]) -> DrawableId {
    let id = client
        .create_drawable_from_template(&ThingTemplate::new())
        .unwrap();
    client
        .find_drawable_by_id_mut(id)
        .unwrap()
        .set_position(Vector3::new(position[0], position[1], position[2]));
    id
}

#[test]
fn native_view_bounds_match_asymmetric_overscan_and_map_extent() {
    let selected = NativeDrawView::from_ground_corners(
        [
            [10., 20., 0.],
            [-30., 50., 0.],
            [40., -60., 0.],
            [5., 7., 0.],
        ],
        [4., 9.],
        [120., 480.],
    )
    .unwrap();
    assert_eq!(selected.region.lo, [-109., -204., -999_879.]);
    assert_eq!(selected.region.hi, [119., 134., 1_000_479.]);
    let mut client = GameClient::new().unwrap();
    let inside = objectless(&mut client, [0., 0., 1_000_000.]);
    let outside = objectless(&mut client, [0., 0., 1_000_480.]);
    assert_eq!(client.native_roster_for_view(&selected).unwrap(), [inside]);
    assert!(client.find_drawable_by_id(outside).is_some());
}

#[test]
fn native_view_region_includes_all_faces_and_corners_in_constructor_order() {
    let region = view([-2., -3., -4.], [5., 6., 7.]);
    let mut client = GameClient::new().unwrap();
    let mut admitted = Vec::new();
    for x in [-2., 5.] {
        for y in [-3., 6.] {
            for z in [-4., 7.] {
                admitted.push(objectless(&mut client, [x, y, z]));
            }
        }
    }
    for axis in 0..3 {
        for bound in [region.region.lo[axis], region.region.hi[axis]] {
            let mut position = [0.; 3];
            position[axis] = bound;
            admitted.push(objectless(&mut client, position));
        }
        for outside in [
            region.region.lo[axis] - 0.001,
            region.region.hi[axis] + 0.001,
        ] {
            let mut position = [0.; 3];
            position[axis] = outside;
            objectless(&mut client, position);
        }
    }
    admitted.reverse();
    assert_eq!(client.native_roster_for_view(&region).unwrap(), admitted);
}

#[test]
fn native_view_negative_bias_retains_inverted_empty_region_and_rejects_nonfinite() {
    let corners = [[0.; 3]; 4];
    let inverted = NativeDrawView::from_ground_corners(corners, [-100., -200.], [0., 0.]).unwrap();
    assert_eq!(inverted.region.lo, [25., 65., -999_999.]);
    assert_eq!(inverted.region.hi, [-25., -125., 999_999.]);
    let mut client = GameClient::new().unwrap();
    objectless(&mut client, [0.; 3]);
    assert!(client.native_roster_for_view(&inverted).unwrap().is_empty());
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(NativeDrawView::from_ground_corners(corners, [invalid, 0.], [0., 0.]).is_err());
        let mut bad_corner = corners;
        bad_corner[3][2] = invalid;
        assert!(NativeDrawView::from_ground_corners(bad_corner, [0.; 2], [0.; 2]).is_err());
        assert!(NativeDrawView::from_ground_corners(corners, [0.; 2], [invalid, 0.]).is_err());
        assert!(
            client
                .native_roster_for_view(&view([invalid, 0., 0.], [1.; 3]))
                .is_err()
        );
    }
}

#[test]
fn native_view_uses_binding_basis_raw_position_and_keeps_same_id_peer_separate() {
    let mut client = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let entry = super::tests::presentation_drawable_sync_for_test(
        41,
        5,
        "ViewOwner",
        true,
        false,
        [2., 90., 3.],
        0.,
    );
    client.sync_presentation_drawables([entry.clone()]).unwrap();
    let mut other = entry;
    other.position = [90.; 3];
    peer.sync_presentation_drawables([other]).unwrap();
    let direct = client.get_drawable_for_object(41).unwrap();
    assert_eq!(peer.get_drawable_for_object(41), Some(direct));
    let core = objectless(&mut client, [2., 90., 3.]);
    let core_inside = objectless(&mut client, [2., 3., 90.]);
    client
        .find_drawable_by_id_mut(direct)
        .unwrap()
        .set_instance_transform(Matrix4::translation(Vector3::new(1000., 1000., 1000.)));
    let region = view([1., 2., 80.], [4., 5., 100.]);
    assert_eq!(
        client.native_roster_for_view(&region).unwrap(),
        [core_inside, direct]
    );
    assert!(peer.native_roster_for_view(&region).unwrap().is_empty());
    assert!(client.find_drawable_by_id(core).is_some());
    let bad = client
        .drawables
        .presentation_direct_drawable_bindings
        .get_mut(&direct)
        .unwrap();
    bad.binding_key.binding_generation += 1;
    bad.binding_key.drawable_id = core;
    assert!(client.native_roster_for_view(&region).is_err());
}

#[test]
fn native_view_multiple_regions_preserve_overlap_and_hidden_objectless_nodes() {
    let mut client = GameClient::new().unwrap();
    let left = objectless(&mut client, [-5., 0., 0.]);
    let overlap = objectless(&mut client, [0., 0., 0.]);
    let right = objectless(&mut client, [5., 0., 0.]);
    client
        .find_drawable_by_id_mut(overlap)
        .unwrap()
        .set_visible(false);
    let first = client
        .native_roster_for_view(&view([-6., -1., -1.], [1.; 3]))
        .unwrap();
    let second = client
        .native_roster_for_view(&view([-1.; 3], [6., 1., 1.]))
        .unwrap();
    assert_eq!(first, [overlap, left]);
    assert_eq!(second, [right, overlap]);
    assert_eq!(
        first.into_iter().chain(second).collect::<Vec<_>>(),
        [overlap, left, right, overlap]
    );
}

#[test]
fn native_view_large_factory_roster_filters_without_id_sort_or_hash_order() {
    let mut client = GameClient::new().unwrap();
    let mut expected = Vec::new();
    for ordinal in 0..64 {
        let inside = ordinal % 3 != 0;
        let id = objectless(&mut client, if inside { [0.; 3] } else { [20.; 3] });
        if inside {
            expected.push(id);
        }
    }
    expected.reverse();
    assert_eq!(
        client
            .native_roster_for_view(&view([-1.; 3], [1.; 3]))
            .unwrap(),
        expected
    );
    assert!(
        NativeDrawView::from_ground_corners([[f32::MAX, 0., 0.]; 4], [f32::MAX, 0.], [0.; 2])
            .is_err()
    );
}
