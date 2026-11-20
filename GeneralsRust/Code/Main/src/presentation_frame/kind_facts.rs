//! Complete immutable host kind facts. The compact entity bank is a separate contract.
use super::{KindOf, RenderableObject};
use crate::game_logic::Object;

pub(super) fn ordered_host_kind_facts(object: &Object) -> Vec<KindOf> {
    let set = object.thing().template.kind_of;
    let mut kinds = Vec::with_capacity(set.len());
    kinds.extend(set.presentation_order());
    kinds
}

pub(super) fn refresh_object_kind_facts(
    renderable: &mut RenderableObject,
    object: &Object,
) -> bool {
    let kinds = object.thing().template.kind_of;
    let kinds_changed = !renderable
        .kind_of
        .iter()
        .copied()
        .eq(kinds.presentation_order());
    if kinds_changed {
        renderable.kind_of.clear();
        renderable.kind_of.extend(kinds.presentation_order());
    }
    let force = object.is_kind_of(KindOf::ForceAttackable);
    let always = object.is_kind_of(KindOf::AlwaysSelectable);
    let reject = object.is_kind_of(KindOf::RejectUnmanned);
    let dirty = kinds_changed
        || renderable.is_force_attackable != force
        || renderable.always_selectable != always
        || renderable.reject_unmanned != reject;
    renderable.is_force_attackable = force;
    renderable.always_selectable = always;
    renderable.reject_unmanned = reject;
    dirty
}
