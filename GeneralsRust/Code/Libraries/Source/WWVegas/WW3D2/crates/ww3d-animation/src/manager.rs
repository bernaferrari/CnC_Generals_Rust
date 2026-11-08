use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::{HAnimClass, hanim_from_prototype};
use ww3d_assets::AssetManager;
use ww3d_assets::prototypes::AnimationPrototype;

/// Runtime animation cache equivalent to the legacy HAnimManagerClass.
#[derive(Default)]
pub struct HAnimManager {
    animations: HashMap<String, Arc<HAnimClass>>,
    missing: HashSet<String>,
}

impl HAnimManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert an animation into the manager. Existing entries are replaced.
    pub fn add_anim(&mut self, anim: Arc<HAnimClass>) {
        self.animations.insert(anim.get_name().to_string(), anim);
    }

    /// Load only when the animation's authored hierarchy is available.
    pub fn load_prototype(
        &mut self,
        proto: &AnimationPrototype,
        assets: &AssetManager,
    ) -> Option<Arc<HAnimClass>> {
        let hierarchy = assets.get_hierarchy_prototype(&proto.hierarchy_name)?;
        let anim = Arc::new(hanim_from_prototype(proto, hierarchy)?);
        self.add_anim(anim.clone());
        Some(anim)
    }

    /// Retrieve an animation by name, cloning the shared reference.
    pub fn get_anim(&self, name: &str) -> Option<Arc<HAnimClass>> {
        self.animations.get(name).cloned()
    }

    /// Peek an animation without cloning the underlying data.
    pub fn peek_anim(&self, name: &str) -> Option<&Arc<HAnimClass>> {
        self.animations.get(name)
    }

    /// Remove all animations from the manager.
    pub fn free_all_anims(&mut self) {
        self.animations.clear();
    }

    pub fn remove_anim(&mut self, name: &str) {
        self.animations.remove(name);
    }

    pub fn contains(&self, name: &str) -> bool {
        self.animations.contains_key(name)
    }

    pub fn len(&self) -> usize {
        self.animations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.animations.is_empty()
    }

    /// Register a missing animation so repeated lookups can be short-circuited.
    pub fn register_missing(&mut self, name: &str) {
        self.missing.insert(name.to_string());
    }

    pub fn is_missing(&self, name: &str) -> bool {
        self.missing.contains(name)
    }

    pub fn reset_missing(&mut self) {
        self.missing.clear();
    }

    /// Iterate over loaded animation names.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Arc<HAnimClass>)> {
        self.animations.iter()
    }

    pub fn load_from_asset_manager(&mut self, assets: &AssetManager) {
        for (_name, prototype) in assets.prototypes() {
            if let Some(anim_proto) = prototype.as_any().downcast_ref::<AnimationPrototype>() {
                self.load_prototype(anim_proto, assets);
            }
        }
    }

    pub fn ensure_from_asset_manager(
        &mut self,
        name: &str,
        assets: &AssetManager,
    ) -> Option<Arc<HAnimClass>> {
        if let Some(existing) = self.get_anim(name) {
            return Some(existing);
        }
        if self.is_missing(name) {
            return None;
        }
        if let Some(proto) = assets.get_prototype_as::<AnimationPrototype>(name) {
            return self.load_prototype(proto, assets);
        }
        self.register_missing(name);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ww3d_assets::prototypes::{AnimationChannelData, HierarchyPrototype};
    use ww3d_core::{W3dPivotStruct, W3dVectorStruct};

    fn hierarchy(name: &str, count: usize) -> HierarchyPrototype {
        let mut result = HierarchyPrototype::new(name.into());
        for index in 0..count {
            result.pivots.push(W3dPivotStruct {
                name: [0; 16],
                parent_idx: if index == 0 { -1 } else { 0 },
                translation: W3dVectorStruct { x: 0.0, y: 0.0, z: 0.0 },
                euler_angles: W3dVectorStruct { x: 0.0, y: 0.0, z: 0.0 },
            });
        }
        result.num_pivots = count as u32;
        result
    }

    #[test]
    fn prototype_loading_requires_named_hierarchy_and_uses_its_complete_size() {
        let mut assets = AssetManager::new();
        let mut proto = AnimationPrototype::new("walk".into(), "unit".into());
        proto.num_frames = 1;
        for pivot in [1, 7] {
            proto.channels.push(AnimationChannelData {
                first_frame: 0, last_frame: 0, vector_len: 1, flags: 0,
                pivot, data: vec![3.0],
            });
        }
        assets.add_prototype("walk".into(), Box::new(proto));
        let mut manager = HAnimManager::new();
        assert!(manager.ensure_from_asset_manager("walk", &assets).is_none());

        assets.add_prototype("unit".into(), Box::new(hierarchy("unit", 5)));
        let anim = manager.ensure_from_asset_manager("walk", &assets).unwrap();
        assert_eq!(anim.num_pivots(), 5);
        assert_eq!(anim.get_translation(1, 0.0).x, 3.0);
        assert!(anim.get_visibility(4, 0.0));
        assert!(!anim.has_translation(7));
    }
}
