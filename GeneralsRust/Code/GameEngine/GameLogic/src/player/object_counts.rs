//! CPP Player1776 / Team1593: template counting against a supplied object owner.
use super::*;
impl Player {
    /// Count this player's roster through an ordinary borrowed store lookup.
    /// Missing IDs stay absent; the lookup is never retained or published.
    pub fn count_objects_by_thing_template_owned<'a>(
        &self,
        templates: &[Arc<dyn ThingTemplate>],
        ignore_dead: Bool,
        ignore_under_construction: Bool,
        counts: &mut [Int],
        mut find: impl FnMut(ObjectID) -> Option<&'a Object>,
    ) {
        counts.fill(0);
        for &id in &self.owned_objects {
            if let Some(object) = find(id).filter(|object| object.get_id() == id) {
                self.count_object_template(
                    object,
                    templates,
                    ignore_dead,
                    ignore_under_construction,
                    counts,
                );
            }
        }
    }

    pub(super) fn count_object_template(
        &self,
        object: &Object,
        templates: &[Arc<dyn ThingTemplate>],
        ignore_dead: Bool,
        ignore_under_construction: Bool,
        counts: &mut [Int],
    ) {
        if ignore_dead && object.is_effectively_dead() {
            return;
        }
        if ignore_under_construction && object.test_status(ObjectStatusTypes::UnderConstruction) {
            return;
        }
        let actual = object.get_template();
        for (template, count) in templates.iter().zip(counts) {
            if actual.is_equivalent_to(template.as_ref()) {
                *count += 1;
                break;
            }
        }
    }
}
