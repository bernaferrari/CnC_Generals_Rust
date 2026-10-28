use game_client_rust::core::game_client::GameClient;
use game_client_rust::radius_decal::LiveTerrainDecalSpec;

fn spec(x: f32) -> LiveTerrainDecalSpec {
    LiveTerrainDecalSpec {
        texture: "same-name.tga".into(),
        radius: 2.0,
        position: [x, 3.0, 4.0],
        color: [255, 255, 255],
        opacity: 1.0,
    }
}

#[test]
fn live_terrain_decals_remain_on_their_receiving_client() {
    let mut first = GameClient::new().unwrap();
    let mut second = GameClient::new().unwrap();
    first
        .borrow_native_drawable_owner()
        .replace_live_terrain_decals(&[spec(10.0)]);
    second
        .borrow_native_drawable_owner()
        .replace_live_terrain_decals(&[spec(20.0)]);

    let first_items = first
        .borrow_native_drawable_owner()
        .collect_projected_decal_items(false);
    let second_items = second
        .borrow_native_drawable_owner()
        .collect_projected_decal_items(false);
    assert_eq!(first_items.len(), 1);
    assert_eq!(second_items.len(), 1);
    assert_eq!(first_items[0].texture_name, "same-name.tga");
    assert_eq!(first_items[0].position.x, 10.0);
    assert_eq!(second_items[0].position.x, 20.0);

    first
        .borrow_native_drawable_owner()
        .replace_live_terrain_decals(&[]);
    assert!(
        first
            .borrow_native_drawable_owner()
            .collect_projected_decal_items(false)
            .is_empty()
    );
    let second_after_first_release = second
        .borrow_native_drawable_owner()
        .collect_projected_decal_items(false);
    assert_eq!(second_after_first_release.len(), 1);
    assert_eq!(second_after_first_release[0].position.x, 20.0);
}
