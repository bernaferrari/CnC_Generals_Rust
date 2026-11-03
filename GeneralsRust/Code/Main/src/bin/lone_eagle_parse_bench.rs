//! Stage timers for Lone Eagle CPU map parse (no full game boot).
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use game_engine::common::common_services::CommonServicesScope;
use game_engine::common::ini::ini_road::TerrainRoadCollection;
use generals_main::game_logic::script_loader::{
    load_chunky_map, parse_blend_tile_data_from_chunky, parse_heightmap_data_from_chunky,
    parse_map_settings_from_chunky, parse_runtime_map_objects_bundle_from_chunky,
    parse_runtime_polygon_triggers_from_chunky, parse_runtime_sides_from_chunky,
    parse_runtime_water_height_from_chunky,
};

fn main() {
    let _common = CommonServicesScope::new();
    let path = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
        )
    });
    if !path.is_file() {
        eprintln!("map missing: {}", path.display());
        std::process::exit(2);
    }

    let started = Instant::now();
    let t0 = Instant::now();
    let chunky = load_chunky_map(path.to_str().unwrap())
        .expect("load")
        .expect("present");
    let load_s = t0.elapsed();

    let t = Instant::now();
    let heightmap = parse_heightmap_data_from_chunky(&chunky).expect("hm");
    let heightmap_s = t.elapsed();

    let t = Instant::now();
    let blend = heightmap
        .as_ref()
        .and_then(|hm| parse_blend_tile_data_from_chunky(&chunky, hm).ok().flatten());
    let blend_s = t.elapsed();

    let t = Instant::now();
    let roads = TerrainRoadCollection::new();
    let (wp, _links, bridges, road_segs) =
        parse_runtime_map_objects_bundle_from_chunky(&chunky, &HashMap::new(), &roads)
            .expect("bundle");
    let objects_s = t.elapsed();

    let t = Instant::now();
    let _ = parse_runtime_water_height_from_chunky(&chunky).expect("water");
    let water_s = t.elapsed();

    let t = Instant::now();
    let polys = parse_runtime_polygon_triggers_from_chunky(&chunky).expect("poly");
    let polygons_s = t.elapsed();

    let t = Instant::now();
    let sides = parse_runtime_sides_from_chunky(&chunky).expect("sides");
    let sides_s = t.elapsed();

    let t = Instant::now();
    let settings = parse_map_settings_from_chunky(&chunky).expect("settings");
    let settings_s = t.elapsed();

    println!(
        "lone_eagle_parse_bench load={:.3}s heightmap={:.3}s blend={:.3}s objects={:.3}s (wp={} bridges={} roads={}) water={:.3}s polygons={:.3}s (n={}) sides={:.3}s (sides={} teams={}) settings={:.3}s (objects={}) total={:.3}s",
        load_s.as_secs_f32(),
        heightmap_s.as_secs_f32(),
        blend_s.as_secs_f32(),
        objects_s.as_secs_f32(),
        wp.len(),
        bridges.len(),
        road_segs.len(),
        water_s.as_secs_f32(),
        polygons_s.as_secs_f32(),
        polys.len(),
        sides_s.as_secs_f32(),
        sides.side_dicts.len(),
        sides.team_dicts.len(),
        settings_s.as_secs_f32(),
        settings.objects.len(),
        started.elapsed().as_secs_f32(),
    );
    let _ = blend;
}
