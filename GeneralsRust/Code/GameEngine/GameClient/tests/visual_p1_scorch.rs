use game_client_rust::terrain::scorch_mesh::{ScorchHeightSource, TerrainScorchBuffer};

struct MockHeight {
    flip: bool,
}

impl ScorchHeightSource for MockHeight {
    fn x_extent(&self) -> i32 {
        4
    }
    fn y_extent(&self) -> i32 {
        4
    }
    fn border_size(&self) -> i32 {
        0
    }
    fn clip_height_world(&self, _x: i32, _y: i32) -> f32 {
        0.0
    }
    fn flip_state(&self, _x: i32, _y: i32) -> bool {
        self.flip
    }
}

#[test]
fn flipped_scorch_cell_uses_opposite_winding() {
    let mut buffer = TerrainScorchBuffer::new();
    buffer.add_scorch([10.0, 10.0, 0.0], 20.0, 0);
    let unflipped = buffer.update_scorches(&MockHeight { flip: false }, 0xFFFFFFFF);
    buffer.scorches_in_buffer = 0;
    let flipped = buffer.update_scorches(&MockHeight { flip: true }, 0xFFFFFFFF);
    assert!(!unflipped.indices.is_empty());
    assert_eq!(unflipped.indices.len(), flipped.indices.len());
    assert_ne!(unflipped.indices, flipped.indices);
}
