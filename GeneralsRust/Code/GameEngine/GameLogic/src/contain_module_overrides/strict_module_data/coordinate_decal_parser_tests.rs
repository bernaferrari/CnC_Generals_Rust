use super::*;
#[test]
fn strict_supply_exit_admits_genuine_labeled_coordinates_and_preserves_axes() {
    use crate::common::Coord3D;
    use crate::object::behavior::supply_center_production_exit_behavior::SupplyCenterProductionExitModuleData;

    // Licensed AirforceGeneral.ini:13973, the complete actual declaration.
    const BODY: &str = r#"    UnitCreatePoint   = X:  0.0  Y:  0.0   Z:0.0
    NaturalRallyPoint = X: 44.0  Y:  0.0   Z:0.0;NaturalRallyPointX must always match GeometryMajorRadius! -ML
  End
"#;
    let mut factory = catalog();
    let parsed = parse(
        &mut factory,
        ModuleType::Behavior,
        "SupplyCenterProductionExitUpdate",
        BODY,
    )
    .expect("genuine full Airforce supply exit declaration");
    let data = parsed
        .as_any()
        .downcast_ref::<SupplyCenterProductionExitModuleData>()
        .expect("actual registered supply-exit data, not an alternate update model");
    assert_eq!(data.unit_create_point, Coord3D::new(0.0, 0.0, 0.0));
    assert_eq!(data.natural_rally_point, Coord3D::new(44.0, 0.0, 0.0));
    for coordinates in [
        "X:1.5 Y:-2.25 Z:30",
        "x=1.5 y=-2.25 z=30",
        "X:=1.5 Y:=-2.25 Z:=30",
        "X : 1.5 Y : -2.25 Z : 30",
    ] {
        let parsed = parse(&mut factory, ModuleType::Behavior, "SupplyCenterProductionExitUpdate", &format!("UnitCreatePoint = {coordinates}\nNaturalRallyPoint = X:-4 Y:5 Z:-6\nGrantTemporaryStealth = 500\nEnd\n"))
            .expect("original coordinate grammar in the actual typed parser");
        let data = parsed
            .as_any()
            .downcast_ref::<SupplyCenterProductionExitModuleData>()
            .unwrap();
        assert_eq!(data.unit_create_point, Coord3D::new(1.5, -2.25, 30.0));
        assert_eq!(data.natural_rally_point, Coord3D::new(-4.0, 5.0, -6.0));
        assert_eq!(data.grant_temporary_stealth_frames, 15);
    }
    for (body, rejected) in [
        ("UnitCreatePoint = X:1 Y:bad Z:3\nEnd\n", "UnitCreatePoint"),
        ("UnitCreatePoint = Y:1 X:2 Z:3\nEnd\n", "UnitCreatePoint"),
        ("NaturalRallyPoint = X:1 Y:2\nEnd\n", "NaturalRallyPoint"),
        ("UnitCreatePoint = 1 2 3\nEnd\n", "UnitCreatePoint"),
        (
            "UnknownSupplyExitField = 73\nEnd\n",
            "UnknownSupplyExitField",
        ),
    ] {
        let error = parse(
            &mut factory,
            ModuleType::Behavior,
            "SupplyCenterProductionExitUpdate",
            body,
        )
        .expect_err("strict original labeled coordinates");
        assert!(
            error.contains("SupplyCenterProductionExitUpdate"),
            "{error}"
        );
        assert!(error.contains(rejected), "{error}");
        assert!(error.contains("line"), "{error}");
    }
}

#[test]
fn strict_dynamic_shroud_admits_real_nested_grid_color_and_keeps_cpp_argb() {
    use crate::object::behavior::dynamic_shroud_clearing_range_update::DynamicShroudClearingRangeUpdateModuleData;

    // Licensed AirforceGeneral.ini:6406. Both the GridDecalTemplate and
    // enclosing Behavior End are retained, including authored // text after A.
    const BODY: &str = r#"    FinalVision = 250.0 
    ChangeInterval = 50
    ShrinkDelay = 2000
    ShrinkTime = 1000
    GrowDelay = 0
    GrowTime = 1000
    GrowInterval = 10 ; Faster than most

    GridDecalTemplate
      Texture           = EXGrid
      Style             = SHADOW_ADDITIVE_DECAL
      OpacityMin        = 50%
      OpacityMax        = 100%
      OpacityThrobTime  = 500
      Color             = R:32 G:64 B:128 A:0 //dim blue, additive
    End

  End
"#;
    let mut factory = catalog();
    let parsed = parse(
        &mut factory,
        ModuleType::Behavior,
        "DynamicShroudClearingRangeUpdate",
        BODY,
    )
    .expect("genuine full Airforce nested shroud declaration");
    let data = parsed
        .as_any()
        .downcast_ref::<DynamicShroudClearingRangeUpdateModuleData>()
        .expect("actual registered shroud data, not an alternate update model");
    assert_eq!(data.final_vision, 250.0);
    assert_eq!(data.change_interval, 2);
    assert_eq!(data.shrink_delay, 60);
    assert_eq!(data.shrink_time, 30);
    assert_eq!(data.grow_delay, 0);
    assert_eq!(data.grow_time, 30);
    assert_eq!(data.grow_interval, 1);
    assert_eq!(data.grid_decal_template.texture_name.as_str(), "EXGrid");
    assert_eq!(data.grid_decal_template.min_opacity, 0.5);
    assert_eq!(data.grid_decal_template.max_opacity, 1.0);
    assert_eq!(data.grid_decal_template.opacity_throb_time, 15);
    assert_eq!(
        data.grid_decal_template.color, 0x0020_4080,
        "C++ GameMakeColor uses ARGB"
    );
    let config = parsed
        .get_dynamic_shroud_clearing_range_update_config()
        .expect("actual presentation configuration keeps typed admitted color");
    assert_eq!(config.grid_decal_template.color, 0x0020_4080);
    for (color, expected) in [
        ("R:32 G:64 B:128 A:0 //dim blue, additive", 0x0020_4080),
        ("R: 32 G: 64 B: 128 A: 7", 0x0720_4080),
        ("r=32 g=64 b=128 a=7", 0x0720_4080),
        ("R : 32 G : 64 B : 128", 0xff20_4080),
        // Existing Rust packed fixture extension; not original INI grammar.
        ("287454020", 0x1122_3344),
        (
            "R:32 G:64 B:128 A:7 ignored after fourth component",
            0x0720_4080,
        ),
    ] {
        let body = format!("GridDecalTemplate\nColor = {color}\nEnd\nEnd\nFollowingBlock\n");
        let parsed = INI::new()
            .with_inline_source(&body, |ini| {
                let parsed = factory
                    .construct(
                        Some(ini),
                        "DynamicShroudClearingRangeUpdate",
                        ModuleType::Behavior,
                    )
                    .expect("ordered split and compact color subtokens");
                ini.read_line()?;
                assert_eq!(
                    ini.get_line_tokens(),
                    ["FollowingBlock"],
                    "both nested Ends retained"
                );
                Ok(parsed)
            })
            .unwrap();
        let data = parsed
            .as_any()
            .downcast_ref::<DynamicShroudClearingRangeUpdateModuleData>()
            .unwrap();
        assert_eq!(data.grid_decal_template.color, expected, "{color}");
    }
    for color in [
        "32 64 128 7",
        "G:64 R:32 B:128",
        "R:32 G:64",
        "R:-1 G:64 B:128",
        "R:256 G:64 B:128",
        "R:32 G:64 B:128 A:bad",
        "R:32 G:64 B:128 A:256",
        "R:32 G:64 B:128 Q:7",
        "R:32 G:64 B:128 //not alpha",
    ] {
        let body = format!("GridDecalTemplate\nColor = {color}\nEnd\nEnd\n");
        let error = parse(
            &mut factory,
            ModuleType::Behavior,
            "DynamicShroudClearingRangeUpdate",
            &body,
        )
        .expect_err("original ordered byte components remain strict");
        assert!(
            error.contains("DynamicShroudClearingRangeUpdate"),
            "{error}"
        );
        assert!(error.contains("Color"), "{error}");
        assert!(error.contains("line"), "{error}");
    }
    let error = parse(
        &mut factory,
        ModuleType::Behavior,
        "DynamicShroudClearingRangeUpdate",
        "GridDecalTemplate\nUnknownGridColorField = 73\nEnd\nEnd\n",
    )
    .expect_err("unknown nested authored fields stay errors");
    assert!(error.contains("UnknownGridColorField = 73"), "{error}");
}

#[test]
fn strict_delivery_decal_uses_the_shared_original_ordered_argb_parser() {
    use crate::object::update::ai_update::deliver_payload_ai_update::DeliverPayloadAIUpdateModuleData;
    let mut factory = catalog();
    // DeliverPayloadAIUpdate.h:211 and DeliverPayloadAIUpdate.cpp:72 both
    // delegate DeliveryDecal to RadiusDecalTemplate::parseRadiusDecalTemplate.
    for (color, expected) in [
        ("R:32 G:64 B:128 A:7", 0x0720_4080),
        ("R: 32 G: 64 B: 128 A: 7 //after alpha", 0x0720_4080),
        ("r=32 g=64 b=128", 0xff20_4080),
        // Existing Rust fixture extension remains distinct from C++ grammar.
        ("287454020", 0x1122_3344),
    ] {
        let body = format!(
            "DropOffset = X:1 Y:2 Z:3\nDeliveryDecal\nColor = {color}\nEnd\nDeliveryDecalRadius = 12.5\nEnd\n"
        );
        let parsed = parse(
            &mut factory,
            ModuleType::Behavior,
            "DeliverPayloadAIUpdate",
            &body,
        )
        .expect("actual registered Delivery Decal shares original ordered ARGB");
        let data = parsed
            .as_any()
            .downcast_ref::<DeliverPayloadAIUpdateModuleData>()
            .expect("actual typed Delivery AI data");
        assert_eq!(
            data.delivery_decal_template.color, expected,
            "C++ GameMakeColor R/G/B order, or explicit packed fixture: {color}"
        );
        assert_eq!(data.delivery_decal_radius, 12.5);
        assert_eq!(
            (data.drop_offset.x, data.drop_offset.y, data.drop_offset.z),
            (1.0, 2.0, 3.0)
        );
    }
    for color in [
        "G:64 R:32 B:128",
        "R:32 G:64",
        "R:32 G:64 B:256",
        "R:32 G:64 B:128 A:invalid",
        "R:32 G:64 B:128 Q:7",
    ] {
        let body = format!("DeliveryDecal\nColor = {color}\nEnd\nEnd\n");
        let error = parse(
            &mut factory,
            ModuleType::Behavior,
            "DeliverPayloadAIUpdate",
            &body,
        )
        .expect_err("Delivery uses the same strict ordered components");
        assert!(error.contains("DeliverPayloadAIUpdate"), "{error}");
        assert!(error.contains("Color"), "{error}");
    }
}
