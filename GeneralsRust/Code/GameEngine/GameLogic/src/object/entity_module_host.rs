//! Detached crate Object module-tag inventory for GameWorld entity install.

use super::Object;

impl Object {
    /// Helper tags then template module tags — same order as `init_modules_for`.
    pub fn installed_module_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .ctor_helper_xfer_tags()
            .into_iter()
            .map(str::to_string)
            .collect();
        for entry in &self.modules {
            let tag = entry.tag();
            if !tag.is_empty() {
                tags.push(tag.to_string());
            } else {
                tags.push(entry.name().to_string());
            }
        }
        tags
    }

    /// Walk real modules in install order and run `on_delete` (no ticking).
    pub fn walk_modules_on_delete(&mut self) -> Vec<String> {
        let tags = self.installed_module_tags();
        self.on_destroy_internal();
        tags
    }
}
