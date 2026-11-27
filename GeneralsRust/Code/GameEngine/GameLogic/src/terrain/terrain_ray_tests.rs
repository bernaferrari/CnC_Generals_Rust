//! Inputs are authored original-C++ component cases, never shared expected outputs.
use super::*;

#[test]
fn malformed_terrain_ray_grid_and_nonfinite_segment_are_errors_not_misses() {
    for (bytes, width, height, border) in [
        (&[][..], 0, 1, 0),
        (&[0][..], 2, 1, 0),
        (&[0][..], 1, 1, -1),
    ] {
        assert!(TerrainHeightSamples::new(bytes, width, height, border).is_err());
    }
    let grid = TerrainHeightSamples::new(&[8; 16], 4, 4, 0).unwrap();
    assert!(
        grid.cast_ray(Vector3::splat(f32::NAN), Vector3::ZERO)
            .is_err()
    );
    assert!(
        !grid
            .cast_ray(
                Vector3::new(-10000., -10000., 30.),
                Vector3::new(-9000., -9000., -20.)
            )
            .unwrap()
            .hit
    );
}

#[test]
fn receiving_cast_reads_restored_heights_and_misses_after_its_owner_resets() {
    let loaded = |sample| {
        let mut map = crate::system::map_loader::MapData::new();
        map.width = 4;
        map.height = 4;
        map.heightmap = vec![sample; 16];
        let mut terrain = super::super::TerrainLogic::new();
        terrain.try_load_map_geometry(map).unwrap();
        terrain
    };
    let start = Vector3::new(15.0, 15.0, 100.0);
    let end = Vector3::new(15.0, 15.0, -20.0);
    let cast = |terrain: &super::super::TerrainLogic| terrain.cast_terrain_ray(start, end).unwrap();
    let mut receiving = loaded(8);
    let peer = loaded(16);
    assert_eq!(cast(&receiving).result.contact_point.z, 5.0);
    assert_eq!(cast(&peer).result.contact_point.z, 10.0);

    receiving
        .try_apply_logic_height_map_bytes_with_effects(&[24; 16], &mut || {})
        .unwrap();
    assert_eq!(cast(&receiving).result.contact_point.z, 15.0);
    assert_eq!(cast(&peer).result.contact_point.z, 10.0);

    receiving.reset();
    assert!(!cast(&receiving).hit);
    assert_eq!(cast(&peer).result.contact_point.z, 10.0);
}

#[test]
fn original_46_cast_ray_input_rows_execute_receiving_borrowed_grid() {
    let expected: Vec<_> = include_str!("terrain_ray_expected.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .collect();
    let mut count = 0;
    println!("TERRAIN_RAY_CPP_ROWS_BEGIN");
    for line in include_str!("terrain_ray_cases.tsv")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let fields: Vec<_> = line.split('|').collect();
        assert_eq!(fields.len(), 7);
        assert_eq!(fields[4], "255");
        let bytes: Vec<u8> = fields[5]
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let values: Vec<_> = fields[6]
            .split(',')
            .map(|word| f32::from_bits(u32::from_str_radix(word, 16).unwrap()))
            .collect();
        assert_eq!(values.len(), 6);
        let grid = TerrainHeightSamples::new(
            &bytes,
            fields[1].parse().unwrap(),
            fields[2].parse().unwrap(),
            fields[3].parse().unwrap(),
        )
        .unwrap();
        let cast = grid
            .cast_ray(
                Vector3::new(values[0], values[1], values[2]),
                Vector3::new(values[3], values[4], values[5]),
            )
            .unwrap();
        let (name, words) = expected[count].split_once('|').unwrap();
        assert_eq!(name, fields[0]);
        let words: Vec<u32> = words.split(',').map(|word| word.parse().unwrap()).collect();
        let mut actual = vec![
            u32::from(cast.hit),
            u32::from(cast.result.start_bad),
            cast.result.fraction.to_bits(),
        ];
        actual.extend(cast.result.contact_point.to_array().map(f32::to_bits));
        actual.extend(cast.result.normal.to_array().map(f32::to_bits));
        assert_eq!(actual, words, "original C++ Cast_Ray case {name}");
        assert_eq!(cast.result.surface_type, if cast.hit { 13 } else { 0 });
        println!(
            "{}",
            serde_json::json!({"case":fields[0],"hit":cast.hit,"start_bad":cast.result.start_bad,"fraction_bits":cast.result.fraction.to_bits(),"point_bits":cast.result.contact_point.to_array().map(f32::to_bits),"normal_bits":cast.result.normal.to_array().map(f32::to_bits),"surface_type":cast.result.surface_type})
        );
        count += 1;
    }
    println!("TERRAIN_RAY_CPP_ROWS_END");
    assert_eq!(count, 46);
}
