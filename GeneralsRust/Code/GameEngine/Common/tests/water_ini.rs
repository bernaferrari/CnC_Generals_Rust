use game_engine::common::ini::ini::{INI, INILoadType};
use game_engine::common::ini::ini_water::{TimeOfDay, parse_color_rgba};

#[test]
fn retail_labelled_water_colors_parse() {
    assert_eq!(
        parse_color_rgba("R:200 G:150 B:100"),
        Ok((200.0, 150.0, 100.0, 255.0))
    );
    assert_eq!(
        parse_color_rgba("R:200 G:150 B:100 A:96"),
        Ok((200.0, 150.0, 100.0, 96.0))
    );
}

#[test]
fn retail_water_blocks_load_through_general_ini_parser() {
    let source = r#"
WaterSet MORNING
  SkyTexture = TSCloudWis.tga
  WaterTexture = TSWater.tga
  Vertex00Color = R:200 G:200 B:200
  Vertex10Color = R:200 G:200 B:200
  Vertex01Color = R:200 G:200 B:200
  Vertex11Color = R:200 G:200 B:200
  DiffuseColor = R:175 G:175 B:175 A:255
  TransparentDiffuseColor = R:150 G:150 B:150 A:128
  UScrollPerMS = 0.002
  VScrollPerMS = 0.002
  SkyTexelsPerUnit = 0.8
  WaterRepeatCount = 32
End

WaterTransparency
  TransparentWaterMinOpacity = 1.0
  TransparentWaterDepth = 3.0
  StandingWaterColor = R:255 G:255 B:255
  StandingWaterTexture = TWWater01.tga
  AdditiveBlending = no
  RadarWaterColor = R:140 G:140 B:255
End
"#;

    let mut ini = INI::new();
    ini.with_inline_source(source, |ini| ini.parse_current_file())
        .expect("retail Water.ini syntax should parse");

    let water = ini.water_definitions().expect("water definitions parsed");
    let morning = water.setting(TimeOfDay::Morning);
    assert_eq!(morning.vertex00_color, (200.0, 200.0, 200.0, 255.0));
    assert_eq!(morning.transparent_diffuse_color.3, 128.0);

    let transparency = water.transparency();
    assert!((transparency.radar_water_color.0 - 140.0 / 255.0).abs() < 1e-5);
    assert!((transparency.radar_water_color.1 - 140.0 / 255.0).abs() < 1e-5);
    assert!((transparency.radar_water_color.2 - 1.0).abs() < 1e-5);
    assert!((transparency.standing_water_color.0 - 1.0).abs() < 1e-5);
    assert!((transparency.standing_water_color.1 - 1.0).abs() < 1e-5);
    assert!((transparency.standing_water_color.2 - 1.0).abs() < 1e-5);
}

#[test]
fn general_ini_water_definitions_belong_to_each_parser() {
    let mut first = INI::new();
    first
        .with_inline_source(
            "WaterSet MORNING\nWaterTexture = First.tga\nEnd\nWaterTransparency\nTransparentWaterDepth = 4\nEnd\n",
            |ini| ini.parse_current_file(),
        )
        .unwrap();
    let mut peer = INI::new();
    peer.with_inline_source(
        "WaterSet MORNING\nWaterTexture = Peer.tga\nEnd\nWaterTransparency\nTransparentWaterDepth = 9\nEnd\n",
        |ini| ini.parse_current_file(),
    )
    .unwrap();
    peer.with_inline_source_and_load_type(
        "WaterTransparency\nTransparentWaterDepth = 13\nEnd\n",
        INILoadType::CreateOverrides,
        |ini| ini.parse_current_file(),
    )
    .unwrap();
    assert_eq!(
        first
            .water_definitions()
            .unwrap()
            .transparency()
            .transparent_water_depth,
        4.0
    );
    assert_eq!(
        peer.water_definitions()
            .unwrap()
            .transparency()
            .transparent_water_depth,
        13.0
    );

    let definitions = first.take_water_definitions().unwrap();
    let mut receiving = INI::new();
    receiving.set_water_definitions(definitions);
    drop(peer);
    assert_eq!(
        receiving
            .water_definitions()
            .unwrap()
            .setting(TimeOfDay::Morning)
            .texture_name
            .as_str(),
        "First.tga"
    );
    let invalid = receiving.with_inline_source(
        "WaterSet MORNING\nWaterTexture = Wrong.tga\nWaterRepeatCount = invalid\nEnd\n",
        |ini| ini.parse_current_file(),
    );
    assert!(invalid.is_err());
    assert_eq!(
        receiving
            .water_definitions()
            .unwrap()
            .setting(TimeOfDay::Morning)
            .texture_name
            .as_str(),
        "First.tga"
    );
}
