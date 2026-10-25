//! Complete immutable host kind facts. The compact entity bank is a separate contract.
use super::{GameLogic, KindOf, PresentationFrame, RenderableObject};
use crate::game_logic::Object;

// Keep the established snapshot list prefix/order. Remaining actual kinds follow
// in ALL_KIND_OF order; this list does not allocate integer-bank bit positions.
const ESTABLISHED_KIND_PREFIX: &[KindOf] = &[
    KindOf::Structure,
    KindOf::Infantry,
    KindOf::Vehicle,
    KindOf::Aircraft,
    KindOf::Projectile,
    KindOf::Resource,
    KindOf::Selectable,
    KindOf::Attackable,
    KindOf::CommandCenter,
    KindOf::Worker,
    KindOf::Hero,
    KindOf::SupplyCenter,
    KindOf::PowerPlant,
    KindOf::FSBarracks,
    KindOf::FSWarFactory,
    KindOf::FSAirfield,
    KindOf::FSInternetCenter,
    KindOf::FSPower,
    KindOf::FSBaseDefense,
    KindOf::FSSupplyDropzone,
    KindOf::FSSupplyCenter,
    KindOf::FSSuperweapon,
    KindOf::FSStrategyCenter,
    KindOf::FSFake,
    KindOf::FSTechnology,
    KindOf::FSBlackMarket,
    KindOf::FSAdvancedTech,
    KindOf::Harvestable,
    KindOf::Powered,
    // Wave 982: IgnoredInGui for host mouseover slaver remap.
    KindOf::IgnoredInGui,
    KindOf::Dozer,
    KindOf::Harvester,
];

pub(super) fn ordered_host_kind_facts(object: &Object) -> Vec<KindOf> {
    let mut kinds = ESTABLISHED_KIND_PREFIX
        .iter()
        .copied()
        .filter(|kind| object.is_kind_of(*kind))
        .collect::<Vec<_>>();
    kinds.extend(
        KindOf::ALL_KIND_OF
            .iter()
            .copied()
            .filter(|kind| object.is_kind_of(*kind) && !ESTABLISHED_KIND_PREFIX.contains(kind)),
    );
    kinds
}

pub(super) fn refresh_object_kind_facts(
    renderable: &mut RenderableObject,
    object: &Object,
) -> bool {
    let kinds = ordered_host_kind_facts(object);
    let force = object.is_kind_of(KindOf::ForceAttackable);
    let always = object.is_kind_of(KindOf::AlwaysSelectable);
    let reject = object.is_kind_of(KindOf::RejectUnmanned);
    let dirty = renderable.kind_of != kinds
        || renderable.is_force_attackable != force
        || renderable.always_selectable != always
        || renderable.reject_unmanned != reject;
    renderable.kind_of = kinds;
    renderable.is_force_attackable = force;
    renderable.always_selectable = always;
    renderable.reject_unmanned = reject;
    dirty
}

impl PresentationFrame {
    pub(super) fn refresh_host_kind_facts(&mut self, logic: &GameLogic) {
        for renderable in &mut self.objects {
            if let Some(object) = logic.host_object(renderable.id) {
                refresh_object_kind_facts(renderable, object);
            }
        }
    }
}
