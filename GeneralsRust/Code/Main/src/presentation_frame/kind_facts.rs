//! Complete immutable host kind facts. The compact entity bank is a separate contract.
use super::KindOf;
use crate::game_logic::Object;

pub(super) fn ordered_host_kind_facts(object: &Object) -> Vec<KindOf> {
    let set = object.thing().template.kind_of;
    let mut kinds = Vec::with_capacity(set.len());
    kinds.extend(set.presentation_order());
    kinds
}
