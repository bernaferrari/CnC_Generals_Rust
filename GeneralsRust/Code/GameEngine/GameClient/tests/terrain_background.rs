use game_client_rust::terrain::{IRegion2D, TerrainBackgroundHeightMap, W3DTerrainBackground};
use game_engine::map_object::MAP_HEIGHT_SCALE;

struct TestMap {
    width: i32,
    height: i32,
    heights: Vec<i32>,
    border: i32,
}

impl TestMap {
    fn flat(width: i32, height: i32, value: i32) -> Self {
        Self {
            width,
            height,
            heights: vec![value; (width * height) as usize],
            border: 0,
        }
    }

    fn set(&mut self, x: i32, y: i32, value: i32) {
        self.heights[(x + y * self.width) as usize] = value;
    }
}

impl TerrainBackgroundHeightMap for TestMap {
    fn x_extent(&self) -> i32 {
        self.width
    }

    fn y_extent(&self) -> i32 {
        self.height
    }

    fn height(&self, x: i32, y: i32) -> i32 {
        self.heights[(x + y * self.width) as usize]
    }

    fn static_diffuse(&self, x: i32, y: i32) -> u32 {
        0x0010_0000 | ((x as u32) << 8) | y as u32
    }

    fn border_size_inline(&self) -> i32 {
        self.border
    }
}

#[test]
fn flat_patch_sets_only_corners_and_emits_two_triangles() {
    let map = TestMap::flat(5, 5, 7);
    let mut background = W3DTerrainBackground::new();
    background.allocate_terrain_buffers(&map, 0, 0, 4);

    let buffers = background
        .do_tessellated_update(IRegion2D::new(0, 0, 4, 4), &map, true)
        .clone();

    assert_eq!(buffers.vertices.len(), 4);
    assert_eq!(buffers.indices, vec![0, 1, 2, 2, 1, 3]);
    assert!(background.get_flip_state(0, 0));
    assert!(background.get_flip_state(4, 0));
    assert!(background.get_flip_state(4, 4));
    assert!(background.get_flip_state(0, 4));
    assert!(!background.get_flip_state(2, 2));
}

#[test]
fn nonflat_patch_recurses_to_required_subpatch_corners() {
    let mut map = TestMap::flat(5, 5, 7);
    map.set(2, 2, 9);
    let mut background = W3DTerrainBackground::new();
    background.allocate_terrain_buffers(&map, 0, 0, 4);

    let buffers = background
        .do_tessellated_update(IRegion2D::new(0, 0, 4, 4), &map, false)
        .clone();

    assert_eq!(buffers.vertices.len(), 25);
    assert_eq!(buffers.indices.len(), 96);
    assert!(background.get_flip_state(2, 2));
    assert_eq!(buffers.indices.len() % 3, 0);
}

#[test]
fn vertices_match_cpp_world_position_uv_and_diffuse() {
    let mut map = TestMap::flat(5, 5, 10);
    map.border = 1;
    let mut background = W3DTerrainBackground::new();
    background.allocate_terrain_buffers(&map, 1, 1, 2);

    let buffers = background
        .do_tessellated_update(IRegion2D::new(1, 1, 3, 3), &map, true)
        .clone();

    let first = buffers.vertices[0];
    assert_eq!(first.diffuse, 0x0010_0101);
    assert_eq!(first.x, 0.0);
    assert_eq!(first.y, 0.0);
    assert_eq!(first.z, 10.0 * MAP_HEIGHT_SCALE);
    assert_eq!(first.u1, 0.0);
    assert_eq!(first.v1, 1.0);
}

#[test]
fn disjoint_partial_update_keeps_existing_buffers() {
    let map = TestMap::flat(5, 5, 3);
    let mut background = W3DTerrainBackground::new();
    background.allocate_terrain_buffers(&map, 0, 0, 4);
    background.do_tessellated_update(IRegion2D::new(0, 0, 4, 4), &map, true);
    let previous = background.buffers().clone();

    let after = background
        .do_tessellated_update(IRegion2D::new(10, 10, 12, 12), &map, true)
        .clone();

    assert_eq!(after, previous);
}
