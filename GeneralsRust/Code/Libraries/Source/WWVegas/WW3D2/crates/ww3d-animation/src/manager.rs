use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::{hanim_from_prototype, HAnimClass};
use ww3d_assets::prototypes::AnimationPrototype;
use ww3d_assets::AssetManager;

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

    /// Create an animation from a prototype and register it.
    pub fn load_prototype(&mut self, proto: &AnimationPrototype) -> Arc<HAnimClass> {
        let anim = Arc::new(hanim_from_prototype(proto));
        self.add_anim(anim.clone());
        anim
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
                self.load_prototype(anim_proto);
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
            return Some(self.load_prototype(proto));
        }
        self.register_missing(name);
        None
    }
}
