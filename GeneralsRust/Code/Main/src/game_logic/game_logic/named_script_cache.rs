//! Bind simulation name receipts to the receiving world's actual objects.
use super::ObjectId;
#[cfg(test)]
use generals_sim::scripting::named_objects::NamedScriptCache;
use generals_sim::scripting::named_objects::NamedScriptTarget;
use std::collections::HashMap;
use std::sync::Arc;

impl super::GameLogic {
    /// Resolve a cache receipt only while its original Object birth is live.
    pub(crate) fn resolve_named_script_target(
        &self,
        target: &NamedScriptTarget,
    ) -> Option<&crate::game_logic::object::Object> {
        self.objects
            .get(&target.id)
            .filter(|object| target.matches(object.id, &object.visual_draw_identity_token))
    }

    /// Project each first literal name and its live birth for script queries.
    pub fn host_named_unit_id_map(&self) -> HashMap<String, u32> {
        self.named_script_cache
            .project_live_ids(|target| self.resolve_named_script_target(target).is_some())
    }

    #[cfg(test)]
    pub(crate) fn seed_named_tombstone_fixture(&mut self, name: &str) {
        self.named_script_cache =
            NamedScriptCache::from_wire(&[(name.to_owned(), 0)], |_| None).unwrap();
    }

    pub(crate) fn initialize_receiving_named_script_cache(&mut self) {
        self.named_script_cache
            .seed(self.objects.registered_values().map(|object| {
                (
                    object.name.clone(),
                    object.id,
                    Arc::clone(&object.visual_draw_identity_token),
                )
            }));
    }
    pub(crate) fn add_receiving_named_script_object(&mut self, id: ObjectId) -> bool {
        let Some(object) = self.objects.get(&id) else {
            return false;
        };
        self.named_script_cache
            .add(&object.name, id, &object.visual_draw_identity_token)
    }
    /// Original transfer never clears the donor's Object.name.
    pub(crate) fn transfer_receiving_script_name(&mut self, name: &str, to: ObjectId) -> bool {
        if name.is_empty() {
            return false;
        }
        let Some(target) = self.objects.get(&to) else {
            return false;
        };
        let named = !target.name.is_empty();
        let birth = Arc::clone(&target.visual_draw_identity_token);
        let old = self.named_script_cache.transfer(name, to, &birth, named);
        let color = old.and_then(|old| {
            self.resolve_named_script_target(&old)
                .map(|object| object.custom_indicator_color)
        });
        let target = self
            .objects
            .get_mut(&to)
            .expect("validated transfer target");
        target.name = name.into();
        // An absent row leaves the target indicator untouched. A matching NULL
        // row likewise has no donor; a live donor may explicitly clear color.
        if let Some(color) = color {
            target.custom_indicator_color = color;
        }
        true
    }
}

#[cfg(test)]
#[path = "named_script_cache_tests.rs"]
mod cache_tests;
