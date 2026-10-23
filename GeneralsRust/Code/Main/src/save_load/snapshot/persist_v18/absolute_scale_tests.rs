use super::*;
use crate::game_logic::{KindOf, Object, Team, ThingTemplate};

fn world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut template = ThingTemplate::new("AbsoluteScaleActor");
    template.set_asset_scale(2.0).add_kind_of(KindOf::Immobile);
    let id = ObjectId(8101);
    world.add_object(Object::new(template, id, Team::USA));
    (world, id)
}

#[test]
fn nonunit_drawable_constructor_and_presentation_use_one_absolute_scale() {
    let (mut world, id) = world();
    assert_eq!(world.host_object(id).unwrap().drawable_instance_scale, 2.0);
    let initial = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 0);
    assert_eq!(
        initial
            .objects
            .iter()
            .find(|object| object.id == id)
            .unwrap()
            .mesh_scale,
        2.0
    );
    assert_eq!(initial.native_ghost_scale_sources[&id].asset_scale, 2.0);
    assert_eq!(initial.native_ghost_scale_sources[&id].instance_scale, 2.0);
    world.host_object_mut(id).unwrap().drawable_instance_scale = 0.2;
    let changed = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 0);
    assert_eq!(
        changed
            .objects
            .iter()
            .find(|object| object.id == id)
            .unwrap()
            .mesh_scale,
        0.2
    );
    assert_eq!(changed.native_ghost_scale_sources[&id].asset_scale, 0.2);
    assert_eq!(changed.native_ghost_scale_sources[&id].instance_scale, 0.2);
    world.host_object_mut(id).unwrap().drawable_instance_scale = 0.0;
    let zero = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 0);
    let input = zero
        .unit_render_inputs()
        .into_iter()
        .find(|input| input.id == id)
        .unwrap();
    assert_eq!(input.mesh_scale, 0.0);
    assert!(input.world_matrix().is_finite());
    assert_eq!(input.world_matrix().x_axis.truncate(), glam::Vec3::ZERO);
    assert!(input.world_matrix_without_geometry_scale().is_finite());
    assert_eq!(zero.native_ghost_scale_sources[&id].asset_scale, 0.0);
}

#[test]
fn historical_multiplier_and_absolute_drawable_capsules_admit_once() {
    let (mut world, id) = world();
    let mut historical = capture_persist_v18(&world);
    historical
        .drawable_xfer
        .iter_mut()
        .find(|entry| entry.object_id == id.0)
        .unwrap()
        .instance_scale = 1.0;
    // The v18 DTO is deliberately unchanged. Versions23..29 select this old
    // multiplier admission only at the outer world restore boundary.
    let encoded = bincode_legacy::serialize(&historical).unwrap();
    let decoded = bincode_legacy::deserialize(&encoded).unwrap();
    restore_persist_v18_with_scale_encoding(&decoded, &mut world, true).unwrap();
    assert_eq!(world.host_object(id).unwrap().drawable_instance_scale, 2.0);
    let absolute = capture_persist_v18(&world);
    let encoded = bincode_legacy::serialize(&absolute).unwrap();
    let decoded = bincode_legacy::deserialize(&encoded).unwrap();
    world.host_object_mut(id).unwrap().drawable_instance_scale = 7.0;
    restore_persist_v18(&decoded, &mut world).unwrap();
    assert_eq!(
        world.host_object(id).unwrap().drawable_instance_scale,
        2.0,
        "new absolute scale must not become four"
    );
}

#[test]
fn version30_absolute_scale_capability_preserves_identical_positional_world_body() {
    use crate::save_load::snapshot::{WorldSnapshot, world_codec::decode_bincode_world_snapshot};
    let mut old = WorldSnapshot::default();
    old.version = 29;
    let old_bytes = bincode_legacy::serialize(&old).unwrap();
    let mut new: WorldSnapshot = bincode_legacy::deserialize(&old_bytes).unwrap();
    new.version = 30;
    let new_bytes = bincode_legacy::serialize(&new).unwrap();
    assert_eq!(
        &old_bytes[4..],
        &new_bytes[4..],
        "scale capability must not alter the positional world body"
    );
    assert_eq!(
        decode_bincode_world_snapshot(&old_bytes).unwrap().version,
        29
    );
    assert_eq!(
        decode_bincode_world_snapshot(&new_bytes).unwrap().version,
        30
    );
    new.version = crate::save_load::snapshot::WORLD_SNAPSHOT_BINCODE_VERSION + 1;
    assert!(decode_bincode_world_snapshot(&bincode_legacy::serialize(&new).unwrap()).is_err());
}

#[cfg(feature = "game_client")]
#[test]
fn foreign_client_scale_cannot_replace_receiving_absolute_host_scale() {
    use game_client::core::game_client::{DrawableId, GameClient};
    use game_client::drawable::BasicDrawable;
    let (mut world, id) = world();
    world.host_object_mut(id).unwrap().drawable_instance_scale = 0.2;
    let mut client = GameClient::new().unwrap();
    let drawable = client
        .register_drawable(Box::new(BasicDrawable::new(DrawableId(0))))
        .unwrap();
    client.bind_drawable_to_object(drawable, id.0).unwrap();
    let captured = capture_persist_v18_with_client(&world, &client);
    assert_eq!(
        captured
            .drawable_xfer
            .iter()
            .find(|entry| entry.object_id == id.0)
            .unwrap()
            .instance_scale,
        0.2
    );
}

#[test]
fn parsed_zero_and_negative_scale_reach_factory_frame_and_ghost_source_without_geometry_changes() {
    for scale in [0.0_f32, -2.0] {
        let mut parser = crate::assets::IniParser::new();
        let source_text = format!(
            "Object AuthoredScaleDomain\nScale = {scale}\nKindOf = IMMOBILE\nGeometry = BOX\nGeometryMajorRadius = 9\nGeometryMinorRadius = 6\nGeometryHeight = 12\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\nEnd"
        );
        parser
            .parse_ini_content(&source_text, "authored-scale-domain.ini")
            .unwrap();
        let definition = parser
            .get_definition("AuthoredScaleDomain")
            .unwrap()
            .clone();
        assert_eq!(definition.scale, scale);
        let mut world = GameLogic::new();
        let template = GameLogic::build_template_from_object_definition(
            "AuthoredScaleDomain",
            &definition,
            None,
        );
        world
            .templates
            .insert("AuthoredScaleDomain".into(), template);
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "authored-scale-domain.ini".into(),
                    source_text,
                )])
                .unwrap(),
            )
            .unwrap();
        let id = world
            .create_object("AuthoredScaleDomain", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let object = world.host_object(id).unwrap();
        assert_eq!(object.get_template().asset_scale, scale);
        assert_eq!(object.drawable_instance_scale, scale);
        let geometry = object.get_template().geometry_info;
        assert_eq!(geometry.major_radius, 9.0);
        assert_eq!(geometry.minor_radius, 6.0);
        assert_eq!(geometry.height, 12.0);
        let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 0);
        let input = frame
            .unit_render_inputs()
            .into_iter()
            .find(|input| input.id == id)
            .unwrap();
        assert_eq!(input.mesh_scale, scale);
        let matrix = input.world_matrix();
        assert!(matrix.is_finite());
        assert_eq!(matrix.x_axis.truncate(), glam::Vec3::X * scale);
        assert_eq!(matrix.y_axis.truncate(), glam::Vec3::Y * scale);
        assert_eq!(matrix.z_axis.truncate(), glam::Vec3::Z * scale);
        assert!(input.world_matrix_without_geometry_scale().is_finite());
        assert_eq!(frame.native_ghost_scale_sources[&id].asset_scale, scale);
        assert_eq!(frame.native_ghost_scale_sources[&id].instance_scale, scale);
        assert_eq!(
            world.host_object(id).unwrap().get_template().geometry_info,
            geometry
        );
    }
    let mut template = ThingTemplate::new("FiniteScalePolicy");
    template.set_asset_scale(-2.0);
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        template.set_asset_scale(invalid);
        assert_eq!(template.asset_scale, -2.0);
    }
    assert_eq!(
        crate::assets::mesh_asset_resolve::mesh_scale_from_template(&template),
        -2.0
    );
    template.asset_scale = f32::NAN;
    assert_eq!(
        crate::assets::mesh_asset_resolve::mesh_scale_from_template(&template),
        1.0
    );
}
