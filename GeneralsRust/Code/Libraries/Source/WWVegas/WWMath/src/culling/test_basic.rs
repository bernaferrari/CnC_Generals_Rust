//! Basic test file to check culling system compilation

use crate::{Vector3, AABox, Frustum};
use std::sync::Arc;

use super::{CullType, CullStats, Cullable, CullCollection, GridCullSystem};

#[derive(Debug)]
struct SimpleTestObject {
    id: u64,
    bbox: AABox,
}

impl SimpleTestObject {
    fn new(id: u64, center: Vector3, extent: Vector3) -> Self {
        Self {
            id,
            bbox: AABox::new(center, extent),
        }
    }
}

impl Cullable for SimpleTestObject {
    fn get_cull_box(&self) -> AABox {
        self.bbox
    }

    fn set_cull_box(&mut self, box_: AABox, _just_loaded: bool) {
        self.bbox = box_;
    }

    fn get_id(&self) -> u64 {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_culling_types() {
        assert_eq!(CullType::Outside as u8, 0);
        assert_eq!(CullType::Intersecting as u8, 1);
        assert_eq!(CullType::Inside as u8, 2);
    }

    #[test]
    fn test_cull_collection() {
        let mut collection = CullCollection::new();
        let obj = Arc::new(SimpleTestObject::new(1, Vector3::ZERO, Vector3::new(1.0, 1.0, 1.0)));
        
        collection.add(obj);
        assert_eq!(collection.len(), 1);
        assert!(!collection.is_empty());
        
        if let Some(first) = collection.first() {
            assert_eq!(first.get_id(), 1);
        }
    }

    #[test]
    fn test_grid_cull_system_basic() {
        let grid = GridCullSystem::new();
        assert_eq!(grid.get_object_count(), 0);
    }
}